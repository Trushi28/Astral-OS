//! GUI Theme - Colors and styling for the desktop environment

/// Theme colors for the GUI
pub struct Theme {
    // Desktop
    pub desktop_bg: u32,
    pub desktop_gradient_top: u32,
    pub desktop_gradient_bottom: u32,
    
    // Taskbar
    pub taskbar_bg: u32,
    pub taskbar_text: u32,
    pub taskbar_button_bg: u32,
    pub taskbar_button_hover: u32,
    pub taskbar_height: u32,
    
    // Window
    pub window_bg: u32,
    pub window_border: u32,
    pub window_title_bg: u32,
    pub window_title_bg_inactive: u32,
    pub window_title_text: u32,
    pub window_title_height: u32,
    
    // Window buttons
    pub button_close: u32,
    pub button_maximize: u32,
    pub button_minimize: u32,
    pub button_hover: u32,
    
    // General
    pub text_primary: u32,
    pub text_secondary: u32,
    pub accent: u32,
    pub shadow: u32,
}

impl Theme {
    /// Modern dark theme (like macOS dark mode)
    pub const DARK: Theme = Theme {
        // Desktop - Deep blue gradient
        desktop_bg: 0x1a1a2e,
        desktop_gradient_top: 0x16213e,
        desktop_gradient_bottom: 0x0f3460,
        
        // Taskbar - Semi-transparent dark
        taskbar_bg: 0x252525,
        taskbar_text: 0xffffff,
        taskbar_button_bg: 0x3d3d3d,
        taskbar_button_hover: 0x4d4d4d,
        taskbar_height: 40,
        
        // Window - Dark with accent
        window_bg: 0x2d2d2d,
        window_border: 0x3d3d3d,
        window_title_bg: 0x383838,
        window_title_bg_inactive: 0x2a2a2a,
        window_title_text: 0xffffff,
        window_title_height: 30,
        
        // macOS-style window buttons
        button_close: 0xff5f57,    // Red
        button_maximize: 0x28c840,  // Green
        button_minimize: 0xfebc2e,  // Yellow
        button_hover: 0x666666,
        
        // General
        text_primary: 0xffffff,
        text_secondary: 0xaaaaaa,
        accent: 0x0a84ff,
        shadow: 0x00000080,
    };
    
    /// Light theme (like Windows 11)
    pub const LIGHT: Theme = Theme {
        // Desktop - Light blue gradient
        desktop_bg: 0x4a90d9,
        desktop_gradient_top: 0x87ceeb,
        desktop_gradient_bottom: 0x4169e1,
        
        // Taskbar
        taskbar_bg: 0xf3f3f3,
        taskbar_text: 0x333333,
        taskbar_button_bg: 0xe5e5e5,
        taskbar_button_hover: 0xd5d5d5,
        taskbar_height: 48,
        
        // Window
        window_bg: 0xffffff,
        window_border: 0xcccccc,
        window_title_bg: 0xfafafa,
        window_title_bg_inactive: 0xf0f0f0,
        window_title_text: 0x333333,
        window_title_height: 32,
        
        // Windows-style buttons
        button_close: 0xe81123,
        button_maximize: 0x00ca4e,
        button_minimize: 0xffbd44,
        button_hover: 0xcccccc,
        
        // General
        text_primary: 0x333333,
        text_secondary: 0x666666,
        accent: 0x0078d4,
        shadow: 0x00000040,
    };
    
    /// Cyberpunk neon theme
    pub const NEON: Theme = Theme {
        // Desktop - Dark with neon accents
        desktop_bg: 0x0d0221,
        desktop_gradient_top: 0x1a0533,
        desktop_gradient_bottom: 0x0d0221,
        
        // Taskbar
        taskbar_bg: 0x150734,
        taskbar_text: 0x00ff88,
        taskbar_button_bg: 0x2a0f4a,
        taskbar_button_hover: 0x3d1666,
        taskbar_height: 44,
        
        // Window
        window_bg: 0x150734,
        window_border: 0xff00ff,
        window_title_bg: 0x2a0f4a,
        window_title_bg_inactive: 0x1a0533,
        window_title_text: 0x00ff88,
        window_title_height: 28,
        
        // Neon buttons
        button_close: 0xff0055,
        button_maximize: 0x00ff88,
        button_minimize: 0xffff00,
        button_hover: 0xff00ff,
        
        // General
        text_primary: 0x00ff88,
        text_secondary: 0x00ccff,
        accent: 0xff00ff,
        shadow: 0xff00ff40,
    };
}

/// Get the current theme (default: Dark)
pub fn current() -> &'static Theme {
    &Theme::DARK
}
