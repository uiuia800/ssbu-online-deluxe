use std::sync::{
    atomic::{AtomicBool, AtomicU16, Ordering},
    LazyLock, RwLock,
};

use serde::{de::IntoDeserializer, Deserialize, Deserializer};
use skyline::hooks::InlineCtx;
use ssbu_pia_interface::StationConnectionManager;
use ultelier::sync_guest::{self, profile::DockedProfile, BufferMode, IndexMode, ResolutionLevel};

use crate::{
    input_poll::InputSnapshot,
    net::{get_match_flag, is_connected, is_in_game, is_valid_online_mode, MatchFlag},
    utils::is_emulator,
};

pub static RENDER_PROFILE_MANAGER: LazyLock<RenderProfileManager> =
    LazyLock::new(|| RenderProfileManager::new());

static RENDER_PROFILE_CONFIG_FILE_PATH: &str = "sd:/ultimate/ssbu_online_deluxe/config.toml";
static RENDER_PROFILE_CONFIG: RwLock<Option<RenderProfileConfig>> = RwLock::new(None);

#[repr(C)]
#[derive(Debug, Default, Deserialize)]
pub struct MatchRenderProfiles {
    singles: RenderProfile,
    doubles: RenderProfile,
}

#[repr(C)]
#[derive(Debug, Default, Deserialize)]
pub struct RenderProfileConfig {
    menu: RenderProfile,
    offline_match: MatchRenderProfiles,
}

#[derive(Debug, Clone)]
pub struct RenderProfileSettings {
    buffer_mode: BufferMode,
    index_mode: IndexMode,
    default_resolution_level: ResolutionLevel,
    dynamic_res_enabled: bool,
    vsync_enabled: bool,
    render_opts_enabled: bool,
    boost_enabled: bool,
}

impl RenderProfileSettings {
    pub fn vanilla() -> Self {
        RenderProfileSettings {
            buffer_mode: BufferMode::Triple,
            index_mode: IndexMode::TwoBehind,
            default_resolution_level: ResolutionLevel::Res1920x1080,
            dynamic_res_enabled: false,
            vsync_enabled: true,
            render_opts_enabled: false,
            boost_enabled: false,
        }
    }
    pub fn less_lag() -> Self {
        let is_emulator = is_emulator();
        RenderProfileSettings {
            buffer_mode: BufferMode::Double,
            index_mode: IndexMode::OneBehind,
            default_resolution_level: ResolutionLevel::Res1920x1080,
            dynamic_res_enabled: !is_emulator,
            vsync_enabled: false,
            render_opts_enabled: true,
            boost_enabled: false,
        }
    }
    pub fn less_lag_ultra() -> Self {
        let is_emulator = is_emulator();
        let default_res = match is_emulator {
            true => ResolutionLevel::Res1920x1080,
            false => ResolutionLevel::Res1024x576,
        };
        RenderProfileSettings {
            buffer_mode: BufferMode::Double,
            index_mode: IndexMode::Immediate,
            default_resolution_level: default_res,
            dynamic_res_enabled: !is_emulator,
            vsync_enabled: false,
            render_opts_enabled: true,
            boost_enabled: false,
        }
    }
    pub fn less_lag_doubles() -> Self {
        RenderProfileSettings {
            buffer_mode: BufferMode::Triple,
            index_mode: IndexMode::OneBehind,
            default_resolution_level: ResolutionLevel::Res1920x1080,
            dynamic_res_enabled: true,
            vsync_enabled: false,
            render_opts_enabled: true,
            boost_enabled: false,
        }
    }
    pub fn buffer_mode(&self) -> BufferMode {
        self.buffer_mode
    }
    pub fn index_mode(&self) -> IndexMode {
        self.index_mode
    }
    pub fn default_resolution_level(&self) -> ResolutionLevel {
        self.default_resolution_level
    }
    pub fn dynamic_res_enabled(&self) -> bool {
        self.dynamic_res_enabled
    }
    pub fn vsync_enabled(&self) -> bool {
        self.vsync_enabled
    }
    pub fn render_opts_enabled(&self) -> bool {
        self.render_opts_enabled
    }
    pub fn boost_enabled(&self) -> bool {
        self.boost_enabled
    }
    pub fn from_render_profile(rp: RenderProfile) -> Self {
        let mut rps = match rp.preset {
            RenderProfilePreset::Vanilla => RenderProfileSettings::vanilla(),
            RenderProfilePreset::LessLag => RenderProfileSettings::less_lag(),
            RenderProfilePreset::LessLagUltra => RenderProfileSettings::less_lag_ultra(),
            RenderProfilePreset::LessLagDoubles => RenderProfileSettings::less_lag_doubles(),
            _ => panic!("Must specific a valid render profile"),
        };
        rps.boost_enabled = rp.boost_enabled;
        rps
    }
    pub fn to_bits(&self) -> u16 {
        (self.buffer_mode as u16)
            | ((self.index_mode as u16) << 2)
            | ((self.default_resolution_level as u16) << 4)
            | ((self.dynamic_res_enabled as u16) << 11)
            | ((self.vsync_enabled as u16) << 12)
            | ((self.render_opts_enabled as u16) << 13)
            | ((self.boost_enabled as u16) << 14)
    }
    pub fn from_bits(bits: u16) -> Option<Self> {
        Some(Self {
            buffer_mode: BufferMode::from_u32((bits & 0b11) as u32)?,
            index_mode: IndexMode::from_u32(((bits >> 2) & 0b11) as u32)?,
            default_resolution_level: ResolutionLevel::from_u32(((bits >> 4) & 0x7f) as u32)?,
            dynamic_res_enabled: ((bits >> 11) & 1) != 0,
            vsync_enabled: ((bits >> 12) & 1) != 0,
            render_opts_enabled: ((bits >> 13) & 1) != 0,
            boost_enabled: ((bits >> 14) & 1) != 0,
        })
    }
}

#[repr(u8)]
#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderProfilePreset {
    #[default]
    Vanilla = 0,
    LessLag = 1,
    #[serde(alias = "LLUltra")]
    LessLagUltra = 2,
    #[serde(alias = "LLDoubles")]
    LessLagDoubles = 3,
    Custom = 4,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RenderProfile {
    preset: RenderProfilePreset,
    boost_enabled: bool,
}

impl RenderProfile {
    pub fn from_settings(rps: &RenderProfileSettings) -> Self {
        let preset = match (
            rps.buffer_mode,
            rps.index_mode,
            rps.vsync_enabled,
            rps.render_opts_enabled,
        ) {
            (BufferMode::Triple, IndexMode::TwoBehind, true, false) => RenderProfilePreset::Vanilla,
            (BufferMode::Double, IndexMode::OneBehind, false, true) => RenderProfilePreset::LessLag,
            (BufferMode::Double, IndexMode::Immediate, false, true) => {
                RenderProfilePreset::LessLagUltra
            }
            (BufferMode::Triple, IndexMode::OneBehind, false, true) => {
                RenderProfilePreset::LessLagDoubles
            }
            _ => RenderProfilePreset::Custom,
        };
        RenderProfile {
            preset,
            boost_enabled: rps.boost_enabled,
        }
    }
}

impl<'de> Deserialize<'de> for RenderProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let render_profile_string = String::deserialize(deserializer)?;
        let render_profile_str = render_profile_string.trim();

        let boost_enabled = render_profile_str.ends_with("+") && is_emulator();
        let render_profile_str = render_profile_str.trim_end_matches("+");
        let preset = RenderProfilePreset::deserialize(render_profile_str.into_deserializer())?;

        Ok(RenderProfile {
            preset,
            boost_enabled,
        })
    }
}

impl std::fmt::Display for RenderProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let preset_str = match self.preset {
            RenderProfilePreset::Vanilla => "Vanilla",
            RenderProfilePreset::LessLag => "LessLag",
            RenderProfilePreset::LessLagUltra => "LLUltra",
            RenderProfilePreset::LessLagDoubles => "LLDoubles",
            RenderProfilePreset::Custom => "Custom",
        };
        let boost_enabled_str = match self.boost_enabled {
            true => "++",
            false => "",
        };
        write!(f, "{}{}", preset_str, boost_enabled_str)
    }
}

pub struct RenderProfileManager {
    auto_mode: AtomicBool,
    active_profile_settings: AtomicU16,
    selected_profile_settings: AtomicU16,
    dirty_flag: AtomicBool,
}

impl RenderProfileManager {
    pub fn new() -> Self {
        RenderProfileManager {
            auto_mode: AtomicBool::new(true),
            active_profile_settings: AtomicU16::new(RenderProfileSettings::vanilla().to_bits()),
            selected_profile_settings: AtomicU16::new(RenderProfileSettings::vanilla().to_bits()),
            dirty_flag: AtomicBool::new(false),
        }
    }
    pub fn selected_render_profile_settings(&self) -> RenderProfileSettings {
        RenderProfileSettings::from_bits(self.selected_profile_settings.load(Ordering::SeqCst))
            .unwrap()
    }
    pub fn active_render_profile_settings(&self) -> RenderProfileSettings {
        RenderProfileSettings::from_bits(self.active_profile_settings.load(Ordering::SeqCst))
            .unwrap()
    }
    pub fn selected_render_profile(&self) -> RenderProfile {
        RenderProfile::from_settings(&self.selected_render_profile_settings())
    }
    pub fn active_render_profile(&self) -> RenderProfile {
        RenderProfile::from_settings(&self.active_render_profile_settings())
    }
    pub fn selected_render_profile_set_boost_enabled(&self, boost_enabled: bool) {
        let mut rps = self.selected_render_profile_settings();
        rps.boost_enabled = boost_enabled && is_emulator();
        self.selected_profile_settings
            .store(rps.to_bits(), Ordering::SeqCst);
    }
    fn select_render_profile_immediate(&self, preset: RenderProfilePreset) {
        let rp = RenderProfile {
            preset: preset,
            boost_enabled: self.selected_render_profile().boost_enabled,
        };
        self.selected_profile_settings.store(
            RenderProfileSettings::from_render_profile(rp).to_bits(),
            Ordering::SeqCst,
        );
    }
    fn apply_profile_settings_immediate(&self, rps: &RenderProfileSettings) {
        self.active_profile_settings
            .store(rps.to_bits(), Ordering::SeqCst);
        self.dirty_flag.store(true, Ordering::SeqCst);
    }
    pub fn apply_selected_profile_settings(&self) {
        self.active_profile_settings.store(
            self.selected_profile_settings.load(Ordering::SeqCst),
            Ordering::SeqCst,
        );
        self.dirty_flag.store(true, Ordering::SeqCst);
    }
    pub fn select_next_render_profile(&self) -> RenderProfile {
        let selected_rp = self.selected_render_profile();
        if self.is_auto_mode() {
            return selected_rp;
        }
        let next_preset = match selected_rp.preset {
            RenderProfilePreset::Vanilla => RenderProfilePreset::LessLag,
            RenderProfilePreset::LessLag => RenderProfilePreset::LessLagUltra,
            RenderProfilePreset::LessLagUltra => RenderProfilePreset::LessLagDoubles,
            RenderProfilePreset::LessLagDoubles => RenderProfilePreset::Vanilla,
            RenderProfilePreset::Custom => RenderProfilePreset::Vanilla,
        };
        let next_rp = RenderProfile {
            preset: next_preset,
            boost_enabled: selected_rp.boost_enabled,
        };
        let next_rps = RenderProfileSettings::from_render_profile(next_rp);
        self.selected_profile_settings
            .store(next_rps.to_bits(), Ordering::SeqCst);
        next_rp
    }
    pub fn select_prev_render_profile(&self) -> RenderProfile {
        let selected_rp = self.selected_render_profile();
        if self.is_auto_mode() {
            return selected_rp;
        }
        let prev_preset = match selected_rp.preset {
            RenderProfilePreset::Vanilla => RenderProfilePreset::LessLagDoubles,
            RenderProfilePreset::LessLag => RenderProfilePreset::Vanilla,
            RenderProfilePreset::LessLagUltra => RenderProfilePreset::LessLag,
            RenderProfilePreset::LessLagDoubles => RenderProfilePreset::LessLagUltra,
            RenderProfilePreset::Custom => RenderProfilePreset::Vanilla,
        };
        let prev_rp = RenderProfile {
            preset: prev_preset,
            boost_enabled: selected_rp.boost_enabled,
        };
        let prev_rps = RenderProfileSettings::from_render_profile(prev_rp);
        self.selected_profile_settings
            .store(prev_rps.to_bits(), Ordering::SeqCst);
        prev_rp
    }
    pub fn is_auto_mode(&self) -> bool {
        self.auto_mode.load(Ordering::SeqCst)
    }
    pub fn set_auto_mode(&self, auto_mode: bool) {
        self.auto_mode.store(auto_mode, Ordering::SeqCst);
        self.auto_select_profile(StationConnectionManager::num_connected_stations());
    }
    pub fn poll(
        &self,
        input_snapshot: &InputSnapshot,
        prev: ninput::Buttons,
        next: ninput::Buttons,
    ) -> bool {
        let boost_buttons = input_snapshot.check_buttons_pressed(&[
            ninput::Buttons::L | ninput::Buttons::R | ninput::Buttons::ZL | ninput::Buttons::X,
            ninput::Buttons::L | ninput::Buttons::R | ninput::Buttons::ZR | ninput::Buttons::X,
        ]);
        if !boost_buttons.is_empty() {
            let rps = self.selected_render_profile_settings();
            self.selected_render_profile_set_boost_enabled(!rps.boost_enabled);
            return true;
        }

        let cycle_buttons = input_snapshot.check_buttons_pressed(&[prev, next]);
        if cycle_buttons == prev {
            if self.is_auto_mode() {
                self.set_auto_mode(false);
                self.select_render_profile_immediate(RenderProfilePreset::LessLagDoubles);
            } else {
                if self.selected_render_profile().preset == RenderProfilePreset::Vanilla {
                    self.set_auto_mode(true);
                } else {
                    self.select_prev_render_profile();
                }
            }
            return true;
        } else if cycle_buttons == next {
            if self.is_auto_mode() {
                self.set_auto_mode(false);
                self.select_render_profile_immediate(RenderProfilePreset::Vanilla);
            } else {
                if self.selected_render_profile().preset == RenderProfilePreset::LessLagDoubles {
                    self.set_auto_mode(true);
                } else {
                    self.select_next_render_profile();
                }
            }
            return true;
        }
        false
    }
    pub fn auto_select_profile(&self, num_opponents: usize) {
        if !self.is_auto_mode() {
            return;
        }
        if num_opponents == 0 {
            self.select_render_profile_immediate(RenderProfilePreset::Vanilla);
            return;
        }
        match (is_emulator(), num_opponents > 2) {
            (_, true) => self.select_render_profile_immediate(RenderProfilePreset::LessLagDoubles),
            (true, _) => self.select_render_profile_immediate(RenderProfilePreset::LessLagUltra),
            (false, _) => self.select_render_profile_immediate(RenderProfilePreset::LessLag),
        };
    }
}

#[skyline::hook(offset = 0x3747518, inline)]
unsafe fn main_loop_hook(_ctx: &InlineCtx) {
    if RENDER_PROFILE_MANAGER
        .dirty_flag
        .swap(false, Ordering::SeqCst)
    {
        let rps = RENDER_PROFILE_MANAGER.active_render_profile_settings();
        let _ = sync_guest::set_default_game_resolution_level(rps.default_resolution_level);
        let _ = sync_guest::set_dynamic_resolution_enabled(rps.dynamic_res_enabled);
        let _ = sync_guest::set_vsync_enabled(rps.vsync_enabled);
        let _ = sync_guest::set_render_opts_enabled(rps.render_opts_enabled);
        let _ = sync_guest::set_buffer_mode(rps.buffer_mode);
        let _ = sync_guest::set_index_mode(rps.index_mode);
        if is_emulator() {
            sync_guest::set_fps_boost_enabled(rps.boost_enabled);
        } else {
            let oc_profile = match (RenderProfile::from_settings(&rps).preset, is_in_game()) {
                (_, false) => DockedProfile::Rest,
                (
                    RenderProfilePreset::Vanilla
                    | RenderProfilePreset::LessLag
                    | RenderProfilePreset::Custom,
                    _,
                ) => DockedProfile::Singles,
                (RenderProfilePreset::LessLagUltra | RenderProfilePreset::LessLagDoubles, _) => {
                    DockedProfile::Ffa
                }
            };
            let _ = sync_guest::profile::apply_docked_profile(oc_profile);
        }
    }
}

pub(crate) fn match_init() {
    let is_valid_online_mode = is_valid_online_mode();
    let is_connected = is_connected();
    let match_flag = get_match_flag();
    let in_real_online_match = is_connected && match_flag != MatchFlag::Training;

    if in_real_online_match {
        if is_valid_online_mode {
            RENDER_PROFILE_MANAGER.apply_selected_profile_settings();
        } else {
            // ssbusync already enforces this, but leaving this here for clarity
            RENDER_PROFILE_MANAGER
                .apply_profile_settings_immediate(&RenderProfileSettings::vanilla());
        }
    } else {
        let rpc = RENDER_PROFILE_CONFIG.read().unwrap();
        let match_rp = rpc
            .as_ref()
            .and_then(|c| match match_flag {
                MatchFlag::Singles | MatchFlag::Training => Some(c.offline_match.singles),
                MatchFlag::Doubles => Some(c.offline_match.doubles),
                MatchFlag::Inactive => None,
            })
            .unwrap_or(RenderProfile::default());
        RENDER_PROFILE_MANAGER.apply_profile_settings_immediate(
            &RenderProfileSettings::from_render_profile(match_rp),
        );
    }
}

pub(crate) fn match_cleanup() {
    let rpc = RENDER_PROFILE_CONFIG.read().unwrap();
    let menu_rp = rpc
        .as_ref()
        .and_then(|c| Some(c.menu))
        .unwrap_or(RenderProfile::default());
    RENDER_PROFILE_MANAGER
        .apply_profile_settings_immediate(&RenderProfileSettings::from_render_profile(menu_rp));
}

fn try_load_config_file() -> std::io::Result<RenderProfileConfig> {
    let config_file = std::path::PathBuf::from(RENDER_PROFILE_CONFIG_FILE_PATH);
    if !config_file.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "SSBU Online Deluxe config file not found",
        ));
    }

    let contents = std::fs::read_to_string(RENDER_PROFILE_CONFIG_FILE_PATH)?;
    let config: RenderProfileConfig = toml::from_str(contents.as_str())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(config)
}

#[no_mangle]
pub extern "C" fn ssbu_online_deluxe_set_render_profile_config(
    render_profile_config: RenderProfileConfig,
) {
    println!("[ssbu-online-deluxe] Setting config from external plugin...");
    let mut rpc = RENDER_PROFILE_CONFIG.write().unwrap();
    *rpc = Some(render_profile_config);
}

pub(super) fn install() {
    let _ = LazyLock::force(&RENDER_PROFILE_MANAGER);
    skyline::install_hook!(main_loop_hook);

    let mut rpc = RENDER_PROFILE_CONFIG.write().unwrap();

    if rpc.is_none() {
        println!("Render profile config not specified. Trying to load from file...");
        let render_profile_config = match try_load_config_file() {
            Err(err) => {
                println!("Unable to load config from file: {}", err);
                println!("Using default config...");
                RenderProfileConfig::default()
            }
            Ok(rpc) => {
                println!("Parsed render profile config file succesfully!");
                rpc
            }
        };
        *rpc = Some(render_profile_config);
    }
    println!("Loaded Render Profile Config:\n{:?}", *rpc);

    let menu_rp = rpc
        .as_ref()
        .and_then(|c| Some(c.menu))
        .unwrap_or(RenderProfile::default());
    RENDER_PROFILE_MANAGER
        .apply_profile_settings_immediate(&RenderProfileSettings::from_render_profile(menu_rp));
}
