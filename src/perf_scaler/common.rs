use std::sync::atomic::{AtomicBool, Ordering};

use smashline::{
    skyline_smash::{
        app::{
            self,
            lua_bind::{SlowModule, WorkModule},
        },
        lib::lua_const::FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID,
    },
    *,
};

use crate::perf_scaler::{pop_dynamic_res_report, push_dynamic_res_report};

static CRITICAL_ATTACK_LANDED: AtomicBool = AtomicBool::new(false);

extern "C" {
    #[link_name = "\u{1}_ZN3lib9SingletonIN3app14FighterManagerEE9instance_E"]
    static mut FIGHTER_MANAGER: *mut app::FighterManager;

    #[link_name = "\u{1}_ZN3app8lua_bind38FighterManager__get_fighter_entry_implEPNS_14FighterManagerENS_14FighterEntryIDE"]
    fn get_fighter_entry(arg1: *mut app::FighterManager, arg2: i32) -> u64;
}

fn is_valid_fighter_entry_id(entry_id: i32) -> bool {
    unsafe {
        if entry_id < 0 || entry_id > 7 {
            return false;
        }
        let entry = get_fighter_entry(FIGHTER_MANAGER, entry_id);
        return entry != 0;
    }
}

unsafe extern "C" fn global_camera_zoom_state_fighter_frame(fighter: &mut L2CFighterCommon) {
    const CRITICAL_HIT_FINISH_COOLDOWN_FRAMES: i32 = 7;
    static mut CRITICAL_HIT_ACTIVE: bool = false;
    static mut CRITICAL_HIT_FINISH_COOLDOWN_FRAMES_LEFT: i32 = CRITICAL_HIT_FINISH_COOLDOWN_FRAMES;
    static mut MAIN_ENTRY_ID: i32 = -1;

    if !is_valid_fighter_entry_id(MAIN_ENTRY_ID) {
        for i in 0..8 {
            if is_valid_fighter_entry_id(i) {
                MAIN_ENTRY_ID = i;
                break;
            }
        }
    }

    let module_accessor = app::sv_system::battle_object_module_accessor(fighter.lua_state_agent);
    let entry_id = WorkModule::get_int(module_accessor, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID);
    if entry_id != MAIN_ENTRY_ID {
        return;
    }

    if !CRITICAL_ATTACK_LANDED.load(Ordering::SeqCst) {
        return;
    }

    let is_slow = SlowModule::is_slow(module_accessor);

    if is_slow {
        CRITICAL_HIT_FINISH_COOLDOWN_FRAMES_LEFT = CRITICAL_HIT_FINISH_COOLDOWN_FRAMES;
        CRITICAL_HIT_ACTIVE = true;
    } else if CRITICAL_HIT_ACTIVE {
        CRITICAL_HIT_FINISH_COOLDOWN_FRAMES_LEFT -= 1;
        println!(
            "[CRITICAL_HIT_DRS] critical hit finish cooldown frames left: {}",
            CRITICAL_HIT_FINISH_COOLDOWN_FRAMES_LEFT
        );
        if CRITICAL_HIT_FINISH_COOLDOWN_FRAMES_LEFT <= 0 {
            CRITICAL_HIT_ACTIVE = false;
            println!("[CRITICAL_HIT_DRS] intensive_frame_end");
            pop_dynamic_res_report();
            CRITICAL_ATTACK_LANDED.store(false, Ordering::SeqCst);
        }
    }
}

#[skyline::hook(replace=app::sv_animcmd::EFFECT_GLOBAL_BACK_GROUND_CUT_IN_CENTER_POS)]
unsafe fn cut_in_center(lua_state: u64) {
    if !CRITICAL_ATTACK_LANDED.swap(true, Ordering::SeqCst) {
        println!("[CRITICAL_HIT_DRS] intensive_frame_start");
        push_dynamic_res_report();
    }
    call_original!(lua_state);
}

pub fn install() {
    skyline::install_hooks!(cut_in_center);
    Agent::new("fighter")
        .on_line(Main, global_camera_zoom_state_fighter_frame)
        .install();
}
