//src/shell/commands.rs
use super::ShellTheme;
use crate::drivers::framebuffer::print_colored;
use core::arch::asm;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

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
        
        "fb" => cmd_fb(args, theme),
        "theme" => cmd_theme(args, theme),
        
        "format" => cmd_format(theme),
        "mount" => cmd_mount(theme),
        "ls" => cmd_ls(),
        "cat" => cmd_cat(args, theme),
        "write" => cmd_write(args, theme),
        "rm" => cmd_rm(args, theme),
        "touch" => cmd_touch(args, theme),
        
        "reboot" => cmd_reboot(),
        "history" => {} // Handled by shell itself
        
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
    crate::println!("  clear    - Clear screen (or Ctrl+L)");
    crate::println!("  info     - System information");
    crate::println!("  mem      - Memory statistics");
    crate::println!("  ps       - Process list");
    crate::println!("  reboot   - Reboot system");
    crate::println!();
    
    print_colored("Shell Navigation:\n", theme.info_color);
    crate::println!("  ↑/↓      - Navigate command history");
    crate::println!("  ←/→      - Move cursor in command line");
    crate::println!("  Ctrl+A   - Jump to start of line");
    crate::println!("  Ctrl+E   - Jump to end of line");
    crate::println!("  Ctrl+U   - Clear line before cursor");
    crate::println!("  Ctrl+K   - Clear line after cursor");
    crate::println!();
    
    print_colored("Reality Engine:\n", theme.reality_color);
    crate::println!("  reality  [status|fork|merge]");
    crate::println!("  causality [show|trace|stats]");
    crate::println!("  dream [status|force|wake]");
    crate::println!("  timeline [show|jump|branch]");
    crate::println!();
    
    print_colored("Display:\n", theme.info_color);
    crate::println!("  fb [stats|test]");
    crate::println!("  theme [reality|dream]");
    crate::println!();
    
    print_colored("Filesystem:\n", theme.info_color);
    crate::println!("  format   - Format PsychicFS");
    crate::println!("  mount    - Mount filesystem");
    crate::println!("  ls       - List files");
    crate::println!("  cat <f>  - Read file");
    crate::println!("  write <f> <text> - Write file");
    crate::println!("  touch <f> - Create file");
    crate::println!("  rm <f>   - Delete file");
}

fn cmd_clear() {
    crate::drivers::framebuffer::clear();
}

fn cmd_info(theme: &ShellTheme) {
    print_colored("╔═══ Astral OS v0.3.0 ═══╗\n", theme.info_color);
    crate::println!("HHDM Offset: 0x{:x}", crate::get_hhdm_offset());
    crate::println!("System Ticks: {}", crate::get_timestamp());
    
    use crate::reality::causality::RealityId;
    use crate::reality::dream::get_system_state;
    crate::println!("Reality ID: {}", RealityId::current().as_u64());
    crate::println!("State: {:?}", get_system_state());
    print_colored("╚═════════════════════════╝\n", theme.info_color);
}

fn cmd_mem(theme: &ShellTheme) {
    let (total, used, free) = crate::memory::frame::get_stats();
    
    print_colored("╔═══ Physical Memory ═══╗\n", theme.info_color);
    crate::println!("  Total frames: {}", total);
    crate::println!("  Used frames:  {}", used);
    crate::println!("  Free frames:  {}", free);
    crate::println!("  Total:        {} MB", total * 4 / 1024);
    crate::println!("  Free:         {} MB", free * 4 / 1024);
    
    let (heap_used, heap_free) = crate::memory::heap::get_stats();
    crate::println!();
    print_colored("╔═══ Kernel Heap ═══╗\n", theme.info_color);
    crate::println!("  Used:  {} KB", heap_used / 1024);
    crate::println!("  Free:  {} KB", heap_free / 1024);
    print_colored("╚═══════════════════╝\n", theme.info_color);
}

fn cmd_ps(theme: &ShellTheme) {
    let table = crate::process::process_table().lock();
    
    print_colored("╔═══ Process Table ═══╗\n", theme.info_color);
    crate::println!("PID  STATE      ");
    crate::println!("───  ─────────");
    
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
    print_colored("╚═════════════════════╝\n", theme.info_color);
}

fn cmd_echo(args: core::str::SplitWhitespace) {
    for word in args {
        crate::print!("{} ", word);
    }
    crate::println!();
}

fn cmd_reality(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    use crate::reality::causality::{RealityId, set_current_reality, get_reality_count, get_total_events};
    
    let subcmd = args.next().unwrap_or("status");
    
    match subcmd {
        "status" => {
            print_colored("╔═══ Reality Status ═══╗\n", theme.reality_color);
            crate::println!("  Current Reality: {}", RealityId::current().as_u64());
            crate::println!("  Total Realities: {}", get_reality_count());
            crate::println!("  Causal Events:   {}", get_total_events());
            
            use crate::reality::dream::get_system_state;
            crate::println!("  System State:    {:?}", get_system_state());
            print_colored("╚══════════════════════╝\n", theme.reality_color);
        }
        "fork" => {
            print_colored("⚡ Forking reality...\n", theme.reality_color);
            let new_id = RealityId::new();
            set_current_reality(new_id);
            print_colored(&format!("✓ Created reality: {}\n", new_id.as_u64()), theme.success_color);
        }
        "merge" => {
            print_colored("⚡ Merging to root reality...\n", theme.reality_color);
            set_current_reality(RealityId::root());
            print_colored("✓ Merged to reality 0\n", theme.success_color);
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
            
            print_colored(&format!("╔═══ Recent {} Events ═══╗\n", events.len()), theme.info_color);
            for event in events.iter().rev() {
                crate::println!("  [{}] {:?} (cause: {:?})", 
                    event.id, 
                    event.event_type,
                    event.cause_id
                );
            }
            print_colored("╚═════════════════════════╝\n", theme.info_color);
        }
        "trace" => {
            if let Some(id_str) = args.next() {
                if let Ok(id) = id_str.parse::<u64>() {
                    let chain = get_causal_chain(id);
                    print_colored(&format!("╔═══ Causal chain for event {} ═══╗\n", id), theme.info_color);
                    for (i, event) in chain.iter().enumerate() {
                        let indent = "  ".repeat(i);
                        crate::println!("{}[{}] {:?}", indent, event.id, event.event_type);
                    }
                    print_colored("╚═════════════════════════════════╝\n", theme.info_color);
                }
            } else {
                crate::println!("Usage: causality trace <event_id>");
            }
        }
        "stats" => {
            print_colored("╔═══ Causal Log Statistics ═══╗\n", theme.info_color);
            crate::println!("  Total events: {}", get_total_events());
            crate::println!("  In memory:    {}", get_event_count());
            crate::println!("  Max capacity: 1024");
            print_colored("╚══════════════════════════════╝\n", theme.info_color);
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
            print_colored("╔═══ Dream Engine Status ═══╗\n", theme.info_color);
            crate::println!("  State:        {:?}", stats.state);
            crate::println!("  Idle Cycles:  {}", stats.idle_cycles);
            crate::println!("  Dream Cycles: {}", stats.total_cycles);
            crate::println!("  Compactions:  {}", stats.compactions);
            crate::println!("  Predictions:  {}", stats.predictions);
            print_colored("╚════════════════════════════╝\n", theme.info_color);
        }
        "force" => {
            print_colored("💤 Forcing dream state...\n", theme.info_color);
            enter_dream_state();
            print_colored("✓ Dream state activated\n", theme.success_color);
        }
        "wake" => {
            print_colored("☀️  Waking system...\n", theme.info_color);
            exit_dream_state();
            print_colored("✓ System awakened\n", theme.success_color);
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
            print_colored("╔═══ Timeline Visualization ═══╗\n", theme.info_color);
            crate::println!("  Current: Reality {}", RealityId::current().as_u64());
            crate::println!("  Total branches: {}", get_reality_count());
            print_colored("╚═══════════════════════════════╝\n", theme.info_color);
        }
        "jump" => {
            if let Some(id_str) = args.next() {
                if let Ok(id) = id_str.parse::<u64>() {
                    let new_reality = RealityId::new();
                    set_current_reality(new_reality);
                    print_colored(&format!("⚡ Jumped to reality {}\n", id), theme.success_color);
                }
            } else {
                crate::println!("Usage: timeline jump <reality_id>");
            }
        }
        "branch" => {
            let new_id = RealityId::new();
            set_current_reality(new_id);
            print_colored(&format!("🌿 Branched to new reality: {}\n", new_id.as_u64()), theme.success_color);
        }
        _ => {
            crate::println!("Usage: timeline <show|jump|branch>");
        }
    }
}

fn cmd_fb(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    let subcmd = args.next().unwrap_or("stats");
    
    match subcmd {
        "stats" => {
            print_colored("╔═══ Framebuffer Statistics ═══╗\n", theme.info_color);
            crate::println!("  Font: noto-sans-mono-bitmap");
            crate::println!("  Size: 20px");
            crate::println!("  Features: Anti-aliasing, Unicode");
            print_colored("╚═══════════════════════════════╝\n", theme.info_color);
        }
        "test" => {
            print_colored("Testing colors...\n", theme.info_color);
            print_colored("■ RED ", 0xFF0000);
            print_colored("■ GREEN ", 0x00FF00);
            print_colored("■ BLUE ", 0x0000FF);
            print_colored("■ CYAN ", 0x00FFFF);
            print_colored("■ MAGENTA ", 0xFF00FF);
            print_colored("■ YELLOW\n", 0xFFFF00);
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
            print_colored("✓ Theme: Reality mode\n", theme.success_color);
        }
        "dream" => {
            *theme = ShellTheme::DREAM;
            print_colored("✓ Theme: Dream mode\n", theme.success_color);
        }
        _ => {
            crate::println!("Available themes: reality, dream");
        }
    }
}

fn cmd_format(theme: &ShellTheme) {
    use crate::interrupts::getchar_blocking;
    
    print_colored("⚠️  WARNING: This will erase all data!\n", theme.error_color);
    crate::print!("Continue? (y/n): ");
    
    let c = getchar_blocking();
    crate::println!();
    
    if c == b'y' || c == b'Y' {
        if crate::fs::fs_format() {
            print_colored("✓ Format complete!\n", theme.success_color);
        } else {
            print_colored("✗ Format failed!\n", theme.error_color);
        }
    } else {
        crate::println!("Cancelled.");
    }
}

fn cmd_mount(theme: &ShellTheme) {
    if crate::fs::fs_mount() {
        print_colored("✓ Filesystem mounted!\n", theme.success_color);
    } else {
        print_colored("✗ Mount failed - try 'format' first\n", theme.error_color);
    }
}

fn cmd_ls() {
    let files = crate::fs::fs_list();
    if files.is_empty() {
        crate::println!("(empty)");
    } else {
        for name in files {
            crate::println!("  📄 {}", name);
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
            print_colored("✗ File not found\n", theme.error_color);
        }
    } else {
        crate::println!("Usage: cat <filename>");
    }
}

fn cmd_write(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    if let Some(name) = args.next() {
        let content: String = args.collect::<Vec<&str>>().join(" ");
        if content.is_empty() {
            crate::println!("Usage: write <filename> <content>");
            return;
        }
        
        if crate::fs::fs_write(name, content.as_bytes()) {
            print_colored("✓ Written successfully\n", theme.success_color);
        } else {
            print_colored("✗ Write failed\n", theme.error_color);
        }
    } else {
        crate::println!("Usage: write <filename> <content>");
    }
}

fn cmd_rm(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    if let Some(name) = args.next() {
        if crate::fs::fs_delete(name) {
            print_colored("✓ Deleted\n", theme.success_color);
        } else {
            print_colored("✗ Delete failed\n", theme.error_color);
        }
    } else {
        crate::println!("Usage: rm <filename>");
    }
}

fn cmd_touch(mut args: core::str::SplitWhitespace, theme: &ShellTheme) {
    if let Some(name) = args.next() {
        if crate::fs::fs_create(name) {
            print_colored("✓ Created\n", theme.success_color);
        } else {
            print_colored("✗ Create failed\n", theme.error_color);
        }
    } else {
        crate::println!("Usage: touch <filename>");
    }
}

fn cmd_reboot() {
    print_colored("🔄 Rebooting...\n", 0xFF6600);
    unsafe {
        asm!("lidt [{}]", in(reg) &0u64, options(nostack));
        asm!("int3");
    }
}