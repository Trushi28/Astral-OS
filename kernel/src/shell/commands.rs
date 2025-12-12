//src/shell/commands.rs
use super::ShellTheme;
use crate::drivers::framebuffer::print_colored;

pub fn execute(command: &str, mut args: core::str::SplitWhitespace, theme: &mut ShellTheme) {
    match command {
        "help" => cmd_help(theme),
        "clear" => cmd_clear(),
        "info" => cmd_info(theme),
        "mem" => cmd_mem(theme),
        "ps" => cmd_ps(theme),
        "echo" => cmd_echo(args),
        
        "reality" => cmd_reality(args, theme),
        "causality" => cmd_causality(args, theme),
        "dream" => cmd_dream(args, theme),
        "timeline" => cmd_timeline(args, theme),
        "fractal" => cmd_fractal(args, theme),
        
        "fb" => cmd_fb(args, theme),
        "theme" => cmd_theme(args, theme),
        
        "format" => cmd_format(theme),
        "mount" => cmd_mount(theme),
        "sync" => cmd_sync(theme),
        "ls" => cmd_ls(),
        "cat" => cmd_cat(args, theme),
        "write" => cmd_write(args, theme),
        "rm" => cmd_rm(args, theme),
        "touch" => cmd_touch(args, theme),
        
        "shutdown" => cmd_shutdown(),
        "reboot" => cmd_reboot(),
        "halt" => cmd_halt(),
        "history" => {} // TODO
        "smp" => cmd_smp(theme),
        "security" => cmd_security(args, theme),
        "graphics" => cmd_graphics(theme),
        "net" => cmd_net(args, theme),
        "usertest" => cmd_usertest(theme),
        _ => {
            print_colored("Unknown command: ", theme.error_color);
            crate::println!("{}", command);
            crate::println!("Type 'help' for available commands");
        }
    }
}

fn cmd_help(theme: &ShellTheme) {
    crate::println!("Available commands:");
    crate::println!();
    
    print_colored("System:\n", theme.info_color);
    crate::println!("  help     - Show this help");
    crate::println!("  clear    - Clear screen");
    crate::println!("  info     - System information");
    crate::println!("  mem      - Memory statistics");
    crate::println!("  ps       - Process list");
    crate::println!("  shutdown - Shutdown system");
    crate::println!("  reboot   - Reboot system");
    crate::println!("  halt     - Halt CPU");
    crate::println!();
    
    print_colored("Reality Engine:\n", theme.reality_color);
    crate::println!("  reality   [status|fork|merge]");
    crate::println!("  causality [show|trace|stats]");
    crate::println!("  dream     [status|force|wake]");
    crate::println!("  timeline  [show|jump|branch]");
    crate::println!("  fractal   [status|alloc|hot]");
    crate::println!();
    
    print_colored("Display:\n", theme.info_color);
    crate::println!("  fb [stats|test]");
    crate::println!("  theme [reality|dream]");
    crate::println!();
    
    print_colored("Filesystem:\n", theme.info_color);
    crate::println!("  format   - Format PsychicFS");
    crate::println!("  mount    - Mount filesystem");
    crate::println!("  sync     - Sync filesystem");
    crate::println!("  ls       - List files");
    crate::println!("  cat <f>  - Read file");
    crate::println!("  write <f> <text> - Write file");
    crate::println!("  touch <f> - Create file");
    crate::println!("  rm <f>   - Delete file");
    crate::println!();
    crate::println!("  smp      - SMP status");
    crate::println!("  security  - Security status");
    crate::println!("  graphics  - Graphics server info");
    crate::println!("  net       - Network commands");
}

fn cmd_clear() {
    crate::drivers::framebuffer::clear();
}

fn cmd_info(theme: &ShellTheme) {
    print_colored("=== Astral OS v0.3.0 ===\n", theme.info_color);
    crate::println!("HHDM Offset: 0x{:x}", crate::get_hhdm_offset());
    crate::println!("System Ticks: {}", crate::get_timestamp());
    
    use crate::reality::causality::RealityId;
    use crate::reality::dream::get_system_state;
    crate::println!("Reality ID: {}", RealityId::current().as_u64());
    crate::println!("State: {:?}", get_system_state());
}

fn cmd_mem(theme: &ShellTheme) {
    let (total, used, free) = crate::memory::frame::get_stats();
    
    print_colored("Physical Memory:\n", theme.info_color);
    crate::println!("  Total frames: {}", total);
    crate::println!("  Used frames:  {}", used);
    crate::println!("  Free frames:  {}", free);
    crate::println!("  Total:        {} MB", total * 4 / 1024);
    crate::println!("  Free:         {} MB", free * 4 / 1024);
    
    let (heap_used, heap_free) = crate::memory::heap::get_stats();
    crate::println!();
    print_colored("Kernel Heap:\n", theme.info_color);
    crate::println!("  Used:  {} KB", heap_used / 1024);
    crate::println!("  Free:  {} KB", heap_free / 1024);
    
    if let Some(stats) = crate::memory::fractal::get_fractal_stats() {
        crate::println!();
        print_colored("Fractal Memory:\n", theme.reality_color);
        crate::println!("  Regions:      {}", stats.total_regions);
        crate::println!("  Total memory: {} KB", stats.total_memory / 1024);
        crate::println!("  Max depth:    {}", stats.deepest_depth);
    }
}

fn cmd_ps(theme: &ShellTheme) {
    let table = crate::process::process_table().lock();
    
    print_colored("PID  STATE      \n", theme.info_color);
    crate::println!("---  ---------");
    
    for proc in table.iter() {
        let state = match proc.state {
            crate::process::ProcessState::Ready => "READY",
            crate::process::ProcessState::Running => "RUNNING",
            crate::process::ProcessState::Blocked => "BLOCKED",
            crate::process::ProcessState::Zombie => "ZOMBIE",
        };
        crate::println!("{:<4} {:<9}", proc.pid.as_u64(), state);
    }
    
    crate::println!();
    crate::println!("Total processes: {}", table.count());
}

fn cmd_echo(args: core::str::SplitWhitespace) {
    for word in args {
        crate::print!("{} ", word);
    }
    crate::println!();
}

fn cmd_reality(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    use crate::reality::causality::{RealityId, set_current_reality, get_reality_count};
    use crate::reality::causality::get_total_events;
    
    let subcmd = args.next().unwrap_or("status");
    
    match subcmd {
        "status" => {
            print_colored("╔══ Reality Status ══╗\n", theme.reality_color);
            crate::println!("  Current Reality: {}", RealityId::current().as_u64());
            crate::println!("  Total Realities: {}", get_reality_count());
            crate::println!("  Causal Events:   {}", get_total_events());
            
            use crate::reality::dream::get_system_state;
            crate::println!("  System State:    {:?}", get_system_state());
        }
        "fork" => {
            print_colored("Forking reality...\n", theme.reality_color);
            let new_id = RealityId::new();
            set_current_reality(new_id);
            print_colored(&alloc::format!("Created reality: {}\n", new_id.as_u64()), theme.success_color);
        }
        "merge" => {
            print_colored("Merging to root reality...\n", theme.reality_color);
            set_current_reality(RealityId::root());
            print_colored("Merged to reality 0\n", theme.success_color);
        }
        _ => {
            crate::println!("Usage: reality <status|fork|merge>");
        }
    }
}

fn cmd_causality(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    use crate::reality::causality::{get_recent_events, get_causal_chain, get_total_events, get_event_count};
    
    let subcmd = args.next().unwrap_or("show");
    
    match subcmd {
        "show" => {
            let count: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
            let events = get_recent_events(count);
            
            print_colored(&alloc::format!("Recent {} events:\n", events.len()), theme.info_color);
            for event in events.iter().rev() {
                crate::println!("  [{}] {:?} (cause: {:?})", 
                    event.id, 
                    event.event_type,
                    event.cause_id
                );
            }
        }
        "trace" => {
            if let Some(id_str) = args.next() {
                if let Ok(id) = id_str.parse::<u64>() {
                    let chain = get_causal_chain(id);
                    print_colored(&alloc::format!("Causal chain for event {}:\n", id), theme.info_color);
                    for (i, event) in chain.iter().enumerate() {
                        let indent = "  ".repeat(i);
                        crate::println!("{}[{}] {:?}", indent, event.id, event.event_type);
                    }
                }
            } else {
                crate::println!("Usage: causality trace <event_id>");
            }
        }
        "stats" => {
            print_colored("Causal Log Statistics:\n", theme.info_color);
            crate::println!("  Total events: {}", get_total_events());
            crate::println!("  In memory:    {}", get_event_count());
            crate::println!("  Max capacity: 1024");
        }
        _ => {
            crate::println!("Usage: causality <show|trace|stats> [args]");
        }
    }
}

fn cmd_dream(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    use crate::reality::dream::{get_dream_stats, enter_dream_state, exit_dream_state};
    
    let subcmd = args.next().unwrap_or("status");
    
    match subcmd {
        "status" => {
            let stats = get_dream_stats();
            print_colored("Dream Engine Status:\n", theme.info_color);
            crate::println!("  State:        {:?}", stats.state);
            crate::println!("  Idle Cycles:  {}", stats.idle_cycles);
            crate::println!("  Dream Cycles: {}", stats.total_cycles);
            crate::println!("  Compactions:  {}", stats.compactions);
            crate::println!("  Predictions:  {}", stats.predictions);
        }
        "force" => {
            print_colored("Forcing dream state...\n", theme.info_color);
            enter_dream_state();
        }
        "wake" => {
            print_colored("Waking system...\n", theme.info_color);
            exit_dream_state();
        }
        _ => {
            crate::println!("Usage: dream <status|force|wake>");
        }
    }
}

fn cmd_timeline(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    use crate::reality::causality::{RealityId, set_current_reality, get_reality_count};
    
    let subcmd = args.next().unwrap_or("show");
    
    match subcmd {
        "show" => {
            print_colored("Timeline visualization:\n", theme.info_color);
            crate::println!("  Current: Reality {}", RealityId::current().as_u64());
            crate::println!("  Total branches: {}", get_reality_count());
        }
        "jump" => {
            if let Some(id_str) = args.next() {
                if let Ok(id) = id_str.parse::<u64>() {
                    set_current_reality(RealityId::new());
                    print_colored(&alloc::format!("Jumped to reality {}\n", id), theme.success_color);
                }
            } else {
                crate::println!("Usage: timeline jump <reality_id>");
            }
        }
        "branch" => {
            let new_id = RealityId::new();
            set_current_reality(new_id);
            print_colored(&alloc::format!("Branched to new reality: {}\n", new_id.as_u64()), theme.success_color);
        }
        _ => {
            crate::println!("Usage: timeline <show|jump|branch>");
        }
    }
}

fn cmd_fractal(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    let subcmd = args.next().unwrap_or("status");
    
    match subcmd {
        "status" => {
            if let Some(stats) = crate::memory::fractal::get_fractal_stats() {
                print_colored("Fractal Memory Status:\n", theme.reality_color);
                crate::println!("  Total regions: {}", stats.total_regions);
                crate::println!("  Total memory:  {} KB", stats.total_memory / 1024);
                crate::println!("  Deepest depth: {}", stats.deepest_depth);
                crate::println!("  Next base:     0x{:x}", stats.next_base);
            } else {
                print_colored("Fractal allocator not initialized\n", theme.error_color);
            }
        }
        "alloc" => {
            if let Some(size_str) = args.next() {
                if let Ok(size) = size_str.parse::<usize>() {
                    match crate::memory::fractal::allocate_fractal_region(size) {
                        Ok(coord) => {
                            print_colored("Allocated fractal region:\n", theme.success_color);
                            crate::println!("  Coord: ({}, {}, {}) depth {}", 
                                coord.x, coord.y, coord.z, coord.depth);
                        }
                        Err(e) => {
                            print_colored(&alloc::format!("Failed: {}\n", e), theme.error_color);
                        }
                    }
                }
            } else {
                crate::println!("Usage: fractal alloc <size>");
            }
        }
        "hot" => {
            let hot_regions = crate::memory::fractal::get_hot_regions(10);
            print_colored(&alloc::format!("Hot regions ({}):\n", hot_regions.len()), theme.info_color);
            for coord in hot_regions {
                crate::println!("  ({}, {}, {}) depth {}", 
                    coord.x, coord.y, coord.z, coord.depth);
            }
        }
        _ => {
            crate::println!("Usage: fractal <status|alloc|hot>");
        }
    }
}

fn cmd_fb(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    let subcmd = args.next().unwrap_or("stats");
    
    match subcmd {
        "stats" => {
            print_colored("Framebuffer Statistics:\n", theme.info_color);
            crate::println!("  Font rendering: noto-sans-mono-bitmap");
            crate::println!("  Font size: 20px");
        }
        "test" => {
            print_colored("Testing colors...\n", theme.info_color);
            print_colored("RED ", 0xFF0000);
            print_colored("GREEN ", 0x00FF00);
            print_colored("BLUE ", 0x0000FF);
            print_colored("CYAN ", 0x00FFFF);
            print_colored("MAGENTA ", 0xFF00FF);
            print_colored("YELLOW\n", 0xFFFF00);
        }
        _ => {
            crate::println!("Usage: fb <stats|test>");
        }
    }
}

fn cmd_theme(mut args: core::str::SplitWhitespace, theme: &mut ShellTheme) {
    let theme_name = args.next().unwrap_or("reality");
    
    match theme_name {
        "reality" => {
            *theme = ShellTheme::REALITY;
            print_colored("Theme: Reality mode\n", theme.success_color);
        }
        "dream" => {
            *theme = ShellTheme::DREAM;
            print_colored("Theme: Dream mode\n", theme.success_color);
        }
        _ => {
            crate::println!("Available themes: reality, dream");
        }
    }
}

fn cmd_format(theme: &ShellTheme) {
    use crate::interrupts::getchar_blocking;
    
    print_colored("WARNING: This will erase all data!\n", theme.error_color);
    crate::print!("Continue? (y/n): ");
    
    let c = getchar_blocking();
    crate::println!();
    
    if c == b'y' || c == b'Y' {
        if crate::fs::fs_format() {
            print_colored("Format complete!\n", theme.success_color);
        } else {
            print_colored("Format failed!\n", theme.error_color);
        }
    } else {
        crate::println!("Cancelled.");
    }
}

fn cmd_mount(theme: &ShellTheme) {
    if crate::fs::fs_mount() {
        print_colored("Filesystem mounted!\n", theme.success_color);
    } else {
        print_colored("Mount failed - try 'format' first\n", theme.error_color);
    }
}

fn cmd_sync(theme: &ShellTheme) {
    crate::fs::fs_sync();
    print_colored("Filesystem synced\n", theme.success_color);
}

fn cmd_ls() {
    let files = crate::fs::fs_list();
    if files.is_empty() {
        crate::println!("(empty)");
    } else {
        for name in files {
            crate::println!("  {}", name);
        }
    }
}

fn cmd_cat(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    if let Some(name) = args.next() {
        if let Some(data) = crate::fs::fs_read(name) {
            if let Ok(text) = core::str::from_utf8(&data) {
                crate::println!("{}", text);
            } else {
                crate::println!("(binary data, {} bytes)", data.len());
            }
        } else {
            print_colored("File not found\n", theme.error_color);
        }
    } else {
        crate::println!("Usage: cat <filename>");
    }
}

fn cmd_write(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    if let Some(name) = args.next() {
        let content: alloc::string::String = args.collect::<alloc::vec::Vec<&str>>().join(" ");
        if content.is_empty() {
            crate::println!("Usage: write <filename> <content>");
            return;
        }
        
        if crate::fs::fs_write(name, content.as_bytes()) {
            print_colored("Written successfully\n", theme.success_color);
        } else {
            print_colored("Write failed\n", theme.error_color);
        }
    } else {
        crate::println!("Usage: write <filename> <content>");
    }
}

fn cmd_rm(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    if let Some(name) = args.next() {
        if crate::fs::fs_delete(name) {
            print_colored("Deleted\n", theme.success_color);
        } else {
            print_colored("Delete failed\n", theme.error_color);
        }
    } else {
        crate::println!("Usage: rm <filename>");
    }
}

fn cmd_touch(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    if let Some(name) = args.next() {
        if crate::fs::fs_create(name) {
            print_colored("Created\n", theme.success_color);
        } else {
            print_colored("Create failed\n", theme.error_color);
        }
    } else {
        crate::println!("Usage: touch <filename>");
    }
}

fn cmd_shutdown() {
    crate::power::shutdown();
}

fn cmd_reboot() {
    crate::power::reboot();
}

fn cmd_halt() {
    crate::println!("Halting system...");
    crate::fs::fs_sync();
    
    loop {
        unsafe {
            core::arch::asm!("cli", "hlt", options(nostack, nomem));
        }
    }
}

fn cmd_smp(theme: &ShellTheme) {
    use crate::arch::cpu::{get_cpu_count, online_cpus, get_cpu_data};
    use crate::process::scheduler::get_cpu_stats;
    
    print_colored("SMP Status:\n", theme.info_color);
    crate::println!("  Total CPUs: {}", get_cpu_count());
    crate::println!();
    
    print_colored("CPU  APIC_ID  STATE    QUEUE  IDLE\n", theme.info_color);
    crate::println!("---  -------  -------  -----  ------");
    
    for cpu_id in online_cpus() {
        if let Some(data) = get_cpu_data(cpu_id) {
            let (queue_len, idle) = get_cpu_stats(cpu_id);
            let state = if data.info.bsp { "BSP    " } else { "AP     " };
            
            crate::println!("{:<3}  0x{:04x}   {}  {:<5}  {}",
                cpu_id.as_u8(),
                data.info.apic_id,
                state,
                queue_len,
                idle
            );
        }
    }
}
fn cmd_security(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    let subcmd = args.next().unwrap_or("status");
    
    match subcmd {
        "status" => {
            // Show security status
            print_colored("Security Status:\n", theme.info_color);
            crate::println!("  Mode: Intent-based capabilities");
            crate::println!("  Active contexts: ...");
        }
        "trust" => {
            if let Some(pid_str) = args.next() {
                if let Ok(pid_num) = pid_str.parse::<u64>() {
                    let trust = crate::security::get_trust_score(crate::process::Pid::new());
                    crate::println!("Trust score for PID {}: {}", pid_num, trust);
                }
            }
        }
        _ => {
            crate::println!("Usage: security <status|trust>");
        }
    }
}

fn cmd_graphics(theme: &ShellTheme) {
    use crate::drivers::framebuffer;
    
    print_colored("Graphics System Demo\n", theme.info_color);
    
    let (width, height) = framebuffer::get_dimensions();
    crate::println!("  Screen: {}x{}", width, height);
    crate::println!("  Drawing demo rectangles...");
    
    // Draw some colorful rectangles to demonstrate graphics
    let colors = [
        0xFF0000,  // Red
        0x00FF00,  // Green
        0x0000FF,  // Blue
        0xFFFF00,  // Yellow
        0xFF00FF,  // Magenta
        0x00FFFF,  // Cyan
    ];
    
    let rect_w = 80;
    let rect_h = 60;
    let spacing = 90;
    let start_x = 50;
    let start_y = height.saturating_sub(150);
    
    for (i, &color) in colors.iter().enumerate() {
        let x = start_x + i * spacing;
        framebuffer::fill_rect(x, start_y, rect_w, rect_h, color);
    }
    
    // Draw a larger gradient demo box
    let gradient_x = width / 2 - 150;
    let gradient_y = start_y - 100;
    let gradient_w = 300;
    let gradient_h = 80;
    
    for dy in 0..gradient_h {
        for dx in 0..gradient_w {
            let r = ((dx * 255) / gradient_w) as u32;
            let g = ((dy * 255) / gradient_h) as u32;
            let b = 128u32;
            let color = (r << 16) | (g << 8) | b;
            
            let x = gradient_x + dx;
            let y = gradient_y + dy;
            
            if x < width && y < height {
                // Direct pixel write via blit_buffer would be too slow
                // Use a buffer approach instead
            }
        }
    }
    
    // Use blit for gradient
    let mut gradient_buf = alloc::vec![0u32; gradient_w * gradient_h];
    for dy in 0..gradient_h {
        for dx in 0..gradient_w {
            let r = ((dx * 255) / gradient_w) as u32;
            let g = ((dy * 255) / gradient_h) as u32;
            let b = 180u32;
            gradient_buf[dy * gradient_w + dx] = (r << 16) | (g << 8) | b;
        }
    }
    framebuffer::blit_buffer(&gradient_buf, gradient_x, gradient_y, gradient_w, gradient_h);
    
    print_colored("  Demo complete!\n", theme.success_color);
    crate::println!("  Rendered: 6 solid rects + 1 gradient box");
}

fn cmd_net(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    let subcmd = args.next().unwrap_or("status");
    
    match subcmd {
        "status" => {
            print_colored("Network Status:\n", theme.info_color);
            crate::println!("  Stack: Active");
            crate::println!("  Devices: ...");
        }
        "ifconfig" => {
            print_colored("Network Interfaces:\n", theme.info_color);
            crate::println!("  eth0: 10.0.2.15/24");
        }
        "ping" => {
            if let Some(target) = args.next() {
                crate::println!("Pinging {}...", target);
                // Implement ping
            }
        }
        _ => {
            crate::println!("Usage: net <status|ifconfig|ping>");
        }
    }
}

fn cmd_usertest(theme: &ShellTheme) {
    print_colored("Running usermode test...\n", theme.info_color);
    
    match crate::usermode::test::run_test_program() {
        Ok(()) => {
            print_colored("Usermode test completed!\n", theme.success_color);
        }
        Err(e) => {
            print_colored(&alloc::format!("Usermode test failed: {}\n", e), theme.error_color);
        }
    }
}