pub mod latency_slider;
pub mod ldn;
pub mod pia;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::net::ldn::interface::{get_network_role, NetworkRole};
use crate::utils::is_emulator;
use skyline::hooks::InlineCtx;
use skyline::nn::ui2d::Pane;
use ssbu_pia_interface::StationConnectionManager;
use ultelier::sync_guest;

static mut LOCAL_ONLINE_CSS_NUM_PANES_ADJUSTED: bool = false;
static mut CURRENT_ARENA_ID: String = String::new();

static ONLINE_ARENA_PANE_HANDLE: AtomicU64 = AtomicU64::new(0);
static LOCAL_ROOM_PANE_HANDLE: AtomicU64 = AtomicU64::new(0);

static IN_GAME: AtomicBool = AtomicBool::new(false);

#[skyline::hook(offset = 0x22d9d10, inline)]
unsafe fn online_melee_any_scene_create(_: &InlineCtx) {
    println!("ONLINE ELITE INIT");
    LOCAL_ROOM_PANE_HANDLE.store(0, Ordering::SeqCst);
    ONLINE_ARENA_PANE_HANDLE.store(0, Ordering::SeqCst);
    update_in_game_flag(false);
}

#[skyline::hook(offset = 0x22d9c40, inline)]
unsafe fn bg_matchmaking_seq(_: &InlineCtx) {
    println!("ONLINE BG MM INIT");
    LOCAL_ROOM_PANE_HANDLE.store(0, Ordering::SeqCst);
    ONLINE_ARENA_PANE_HANDLE.store(0, Ordering::SeqCst);
    update_in_game_flag(false);
}

#[skyline::hook(offset = 0x235a650, inline)]
unsafe fn main_menu(_: &InlineCtx) {
    println!("MAIN MENU INIT");
    LOCAL_ROOM_PANE_HANDLE.store(0, Ordering::SeqCst);
    ONLINE_ARENA_PANE_HANDLE.store(0, Ordering::SeqCst);
    update_in_game_flag(false);
}

#[skyline::hook(offset = 0x22d9cf4, inline)]
unsafe fn arena_seq(_: &InlineCtx) {
    println!("ONLINE ARENA INIT");
}

#[skyline::hook(offset = 0x18881f0, inline)]
unsafe fn online_arena_update_room_hook(_: &skyline::hooks::InlineCtx) {
    let pane_handle = ONLINE_ARENA_PANE_HANDLE.load(Ordering::SeqCst) as *mut u64 as *mut Pane;
    if !pane_handle.is_null() {
        crate::ui::native::update_online_arena_ui(pane_handle, CURRENT_ARENA_ID.clone());
    }
}

#[skyline::hook(offset = 0x1887b1c, inline)]
unsafe fn online_arena_set_room_id(ctx: &skyline::hooks::InlineCtx) {
    println!("ONLINE ARENA INIT");
    let panel = *((*((ctx.registers[0].x() + 8) as *const u64) + 0x10) as *const u64);
    ONLINE_ARENA_PANE_HANDLE.store(panel, Ordering::SeqCst);
    CURRENT_ARENA_ID = String::from_utf16(std::slice::from_raw_parts(
        ctx.registers[3].x() as *const u16,
        5,
    ))
    .unwrap();
    update_in_game_flag(false);
}

// called on local online menu init
#[skyline::hook(offset = 0x1bd45e0, inline)]
unsafe fn store_local_menu_pane(ctx: &InlineCtx) {
    println!("LOCAL ONLINE INIT");
    update_in_game_flag(false);
    LOCAL_ONLINE_CSS_NUM_PANES_ADJUSTED = false;
    let handle = *((*((ctx.registers[0].x() + 8) as *const u64) + 0x10) as *const u64);
    LOCAL_ROOM_PANE_HANDLE.store(handle, Ordering::SeqCst);
    update_in_game_flag(false);
}

#[skyline::hook(offset = 0x1bd7a80, inline)]
unsafe fn update_local_menu(_: &InlineCtx) {
    let pane_handle = LOCAL_ROOM_PANE_HANDLE.load(Ordering::SeqCst) as *mut u64 as *mut Pane;
    if !pane_handle.is_null() {
        crate::ui::native::update_local_online_ui(pane_handle);
    }
}

#[skyline::hook(offset = 0x1a26200)]
unsafe fn css_player_pane_num_changed(param_1: i64, prev_num: i32, changed_by_player: u32) {
    if is_local_online_mode()
        && !LOCAL_ONLINE_CSS_NUM_PANES_ADJUSTED
        && changed_by_player == 0
        && get_network_role() == NetworkRole::Host
    {
        LOCAL_ONLINE_CSS_NUM_PANES_ADJUSTED = true;
        *((param_1 + 0x160) as *mut i32) = 2;
    }
    update_in_game_flag(false);
    call_original!(param_1, prev_num, changed_by_player);
}

#[skyline::hook(offset = 0x1a12f60)]
unsafe fn update_css(arg: u64) {
    if is_valid_online_mode() {
        let banner_pane1_ptr =
            (*((*((arg + 0xe58) as *const u64) + 0x10) as *const u64)) as *mut Pane;
        crate::ui::native::update_css_ui(banner_pane1_ptr);
    }
    call_original!(arg);
}

fn update_in_game_flag(new_in_game_flag: bool) {
    if IN_GAME
        .compare_exchange(
            !new_in_game_flag,
            new_in_game_flag,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_ok()
    {
        println!("UPDATE IN GAME STATUS: {}", new_in_game_flag);
        if new_in_game_flag {
            if is_connected() && is_valid_online_mode() {
                crate::render::profile::match_init();
                crate::perf_scaler::match_init();
            } else {
                if !is_emulator() {
                    sync_guest::profile::apply_singles();
                }
            }
        } else {
            latency_slider::match_cleanup();
            crate::render::profile::match_cleanup();
            crate::perf_scaler::match_cleanup();
        }
    }
}

#[inline]
pub fn is_local_online_mode() -> bool {
    return LOCAL_ROOM_PANE_HANDLE.load(Ordering::SeqCst) > 0;
}

#[inline]
pub fn is_online_arena_mode() -> bool {
    return ONLINE_ARENA_PANE_HANDLE.load(Ordering::SeqCst) > 0;
}

#[inline]
pub fn is_valid_online_mode() -> bool {
    #[cfg(feature = "dummy_connection")]
    return true;

    #[cfg(not(feature = "dummy_connection"))]
    return is_online_arena_mode() || is_local_online_mode();
}

#[inline]
pub fn is_connected() -> bool {
    return StationConnectionManager::is_connected();
}

#[inline]
pub fn is_in_game() -> bool {
    IN_GAME.load(Ordering::SeqCst)
}

#[inline]
pub fn is_in_valid_online_game() -> bool {
    is_valid_online_mode() && is_in_game() && is_connected()
}

#[skyline::hook(offset = 0x25d8e00)]
unsafe fn on_stage_presetup(stage_base: u64) {
    call_original!(stage_base);

    let stage_id = *((stage_base + 8) as *mut u32);

    // result stage (normal) == 310
    // result stage (sephiroth) == 354
    let is_result_stage = stage_id == 310 || stage_id == 354;
    if is_result_stage {
        update_in_game_flag(false);
        println!("MATCH END");
        return;
    }

    // ignore waiting room stage
    let is_waiting_room_stage = stage_id == 311;
    if is_waiting_room_stage {
        update_in_game_flag(false);
        return;
    }

    update_in_game_flag(true);
    println!("MATCH START: STAGE_ID={}", stage_id);
}

//result stage ui
//#[skyline::hook(offset = 0x1d68b94, inline)]
//unsafe fn on_match_end2(_: &InlineCtx) {
//    update_in_game_flag(false);
//}

//#[skyline::hook(offset = 0x1344cf0)]
//unsafe fn on_match_start(arg1: u64, arg2: u64) {
//    call_original!(arg1, arg2);
//
//    let base_addr = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64;
//    let stage_id = *((base_addr + 0x52c45d0) as *mut u32);
//
//    // Ignore waiting room
//    let in_actual_match = stage_id != 311;
//    update_in_game_flag(in_actual_match);
//
//    println!("MATCH START");
//}

// 7102ef5720: Result stage presetup
//#[skyline::hook(offset = 0x2ef5724, inline)]
//unsafe fn on_match_end(_: &InlineCtx) {
//    update_in_game_flag(false);
//    println!("MATCH END");
//}

// 7102ef5b90: Result stage presetup (sephiroth only)
//#[skyline::hook(offset = 0x2ef5b94, inline)]
//unsafe fn on_match_end2(_: &InlineCtx) {
//    update_in_game_flag(false);
//    println!("MATCH END (SEPHIROTH)");
//}

pub fn install() {
    skyline::install_hooks!(
        online_melee_any_scene_create,
        bg_matchmaking_seq,
        main_menu,
        online_arena_set_room_id,
        online_arena_update_room_hook,
        store_local_menu_pane,
        update_local_menu,
        update_css,
        css_player_pane_num_changed,
        on_stage_presetup,
        arena_seq,
    );
    latency_slider::install();
    pia::install();
}
