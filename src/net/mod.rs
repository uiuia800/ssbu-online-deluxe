pub mod latency_slider;
pub mod ldn;
pub mod pia;

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

use crate::net::ldn::interface::{get_network_role, NetworkRole};
use skyline::hooks::InlineCtx;
use skyline::nn::ui2d::Pane;
use ssbu_pia_interface::StationConnectionManager;

static mut LOCAL_ONLINE_CSS_NUM_PANES_ADJUSTED: bool = false;
static mut CURRENT_ARENA_ID: String = String::new();

static ONLINE_ARENA_PANE_HANDLE: AtomicU64 = AtomicU64::new(0);
static LOCAL_ROOM_PANE_HANDLE: AtomicU64 = AtomicU64::new(0);

static MATCH_FLAG: AtomicU8 = AtomicU8::new(MatchFlag::Inactive as u8);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchFlag {
    Inactive = 0,
    Singles = 1,
    Doubles = 2,
    Training = 3,
}

extern "C" {
    #[link_name = "\u{1}_ZN3app9smashball16is_training_modeEv"]
    pub fn is_training_mode() -> bool;
}
extern "C" {
    #[link_name = "\u{1}_ZN3app7fighter23get_fighter_entry_countEv"]
    pub fn get_fighter_entry_count() -> i32;
}

#[skyline::hook(offset = 0x22d9d10, inline)]
unsafe fn online_melee_any_scene_create(_: &InlineCtx) {
    println!("ONLINE ELITE INIT");
    LOCAL_ROOM_PANE_HANDLE.store(0, Ordering::SeqCst);
    ONLINE_ARENA_PANE_HANDLE.store(0, Ordering::SeqCst);
    update_match_flag(MatchFlag::Inactive);
}

#[skyline::hook(offset = 0x22d9c40, inline)]
unsafe fn bg_matchmaking_seq(_: &InlineCtx) {
    println!("ONLINE BG MM INIT");
    LOCAL_ROOM_PANE_HANDLE.store(0, Ordering::SeqCst);
    ONLINE_ARENA_PANE_HANDLE.store(0, Ordering::SeqCst);
    update_match_flag(MatchFlag::Inactive);
}

#[skyline::hook(offset = 0x235a650, inline)]
unsafe fn main_menu(_: &InlineCtx) {
    println!("MAIN MENU INIT");
    LOCAL_ROOM_PANE_HANDLE.store(0, Ordering::SeqCst);
    ONLINE_ARENA_PANE_HANDLE.store(0, Ordering::SeqCst);
    update_match_flag(MatchFlag::Inactive);
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
    update_match_flag(MatchFlag::Inactive);
}

// called on local online menu init
#[skyline::hook(offset = 0x1bd45e0, inline)]
unsafe fn store_local_menu_pane(ctx: &InlineCtx) {
    println!("LOCAL ONLINE INIT");
    update_match_flag(MatchFlag::Inactive);
    LOCAL_ONLINE_CSS_NUM_PANES_ADJUSTED = false;
    let handle = *((*((ctx.registers[0].x() + 8) as *const u64) + 0x10) as *const u64);
    LOCAL_ROOM_PANE_HANDLE.store(handle, Ordering::SeqCst);
    update_match_flag(MatchFlag::Inactive);
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
    update_match_flag(MatchFlag::Inactive);
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

fn update_match_flag(match_flag: MatchFlag) {
    let prev = MATCH_FLAG.swap(match_flag as u8, Ordering::SeqCst);
    if prev != match_flag as u8 {
        println!("UPDATE MATCH FLAG: {:?}", match_flag);
        if match_flag != MatchFlag::Inactive {
            crate::render::profile::match_init();
            crate::perf_scaler::match_init();
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
    MATCH_FLAG.load(Ordering::SeqCst) != MatchFlag::Inactive as u8
}

#[inline]
pub fn is_in_training_game() -> bool {
    let match_flag = MATCH_FLAG.load(Ordering::SeqCst);
    match_flag == MatchFlag::Training as u8
}

#[inline]
pub fn is_in_real_game() -> bool {
    let match_flag = MATCH_FLAG.load(Ordering::SeqCst);
    match_flag == MatchFlag::Singles as u8 || match_flag == MatchFlag::Doubles as u8
}

#[inline]
pub fn get_match_flag() -> MatchFlag {
    match MATCH_FLAG.load(Ordering::SeqCst) {
        1 => MatchFlag::Singles,
        2 => MatchFlag::Doubles,
        3 => MatchFlag::Training,
        _ => MatchFlag::Inactive,
    }
}

#[inline]
pub fn is_in_valid_online_game() -> bool {
    is_valid_online_mode() && is_in_real_game() && is_connected()
}

#[skyline::hook(offset = 0x25d8e18, inline)]
unsafe fn on_stage_presetup(ctx: &InlineCtx) {
    let stage_base = ctx.registers[0].x();
    let stage_id = *((stage_base + 8) as *mut u32);

    let is_training_mode = is_training_mode();
    let is_waiting_room_stage = stage_id == 311;

    println!(
        "STAGE PRESETUP: STAGE_ID={}, IS_TRAINING_MODE={}",
        stage_id, is_training_mode
    );

    // result stage (normal) == 310
    // result stage (sephiroth) == 354
    let is_result_stage = stage_id == 310 || stage_id == 354;
    if is_result_stage {
        update_match_flag(MatchFlag::Inactive);
        return;
    }

    if is_training_mode || is_waiting_room_stage {
        update_match_flag(MatchFlag::Training);
        return;
    }

    let fighter_entry_count = get_fighter_entry_count();
    let match_flag = if fighter_entry_count > 2 {
        MatchFlag::Doubles
    } else {
        MatchFlag::Singles
    };
    update_match_flag(match_flag);
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
