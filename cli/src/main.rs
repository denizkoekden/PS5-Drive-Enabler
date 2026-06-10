use clap::{Parser, Subcommand};
use ps5de_core::{disk::Risk, Disk, ProgressEvent};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

/// PS5 Drive Enabler: write the Gen3 enabler image to an NVMe drive so the
/// PS5 accepts it for storage expansion regardless of its PCIe generation.
#[derive(Parser)]
#[command(name = "ps5-drive-enabler", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List all detected disks so you can identify the target NVMe.
    List,
    /// Write the enabler image to a disk (ERASES the first 2 MB of that disk).
    Flash(FlashArgs),
    /// Read the first 2 MB back and confirm it matches the enabler image.
    Verify(VerifyArgs),
}

#[derive(clap::Args)]
struct FlashArgs {
    /// Target disk id or path (see `list`), e.g. disk4, nvme0n1, or 2 on Windows.
    #[arg(short, long)]
    device: String,
    /// Use a custom image file instead of the built-in one.
    #[arg(long)]
    image: Option<PathBuf>,
    /// Skip the interactive confirmation prompt.
    #[arg(short = 'y', long)]
    yes: bool,
    /// Allow writing to internal disks (the system disk is always refused).
    #[arg(long)]
    force: bool,
    /// Do not read the data back to verify after writing.
    #[arg(long)]
    no_verify: bool,
}

#[derive(clap::Args)]
struct VerifyArgs {
    /// Target disk id or path (see `list`).
    #[arg(short, long)]
    device: String,
    /// Compare against a custom image file instead of the built-in one.
    #[arg(long)]
    image: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Command::List) => cmd_list(),
        Some(Command::Flash(args)) => cmd_flash(args),
        Some(Command::Verify(args)) => cmd_verify(args),
        None => {
            print_intro();
            Ok(())
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\nError: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_intro() {
    println!(
        "PS5 Drive Enabler {}\n\n\
         This tool writes a 2 MB \"Gen3 enabler\" image to the start of an NVMe\n\
         drive so the PS5 skips its PCIe Gen4 speed check.\n\n\
         Typical usage:\n\
         \x20 1. Put the NVMe drive in a USB enclosure and connect it.\n\
         \x20 2. ps5-drive-enabler list                # find the drive\n\
         \x20 3. ps5-drive-enabler flash --device <id> # write the enabler\n\n\
         Run with administrator/root rights. Use --help for all options.",
        env!("CARGO_PKG_VERSION")
    );
}

fn cmd_list() -> ps5de_core::Result<()> {
    let disks = ps5de_core::list_disks()?;
    if disks.is_empty() {
        println!("No disks detected.");
        return Ok(());
    }
    println!(
        "{:<10} {:<28} {:>11} {:<10} TYPE",
        "ID", "MODEL", "SIZE", "BUS"
    );
    println!("{}", "-".repeat(78));
    for d in &disks {
        println!(
            "{:<10} {:<28} {:>11} {:<10} {}",
            d.id,
            truncate(&d.model, 28),
            d.size_pretty(),
            truncate(&d.bus, 10),
            risk_label(d.risk()),
        );
    }
    println!(
        "\nPick the EXTERNAL drive that matches your NVMe's size and model.\n\
         Then run:  ps5-drive-enabler flash --device <ID>"
    );
    Ok(())
}

fn cmd_flash(args: FlashArgs) -> ps5de_core::Result<()> {
    let disk = ps5de_core::find_disk(&args.device)?;
    let image = match &args.image {
        Some(p) => ps5de_core::image::load_from_path(p)?,
        None => ps5de_core::embedded_image().to_vec(),
    };

    print_target(&disk);

    // Safety gates. The system disk is refused unconditionally.
    match disk.risk() {
        Risk::System => {
            return Err(ps5de_core::Error::Other(format!(
                "'{}' is a SYSTEM disk (it holds your operating system). \
                 Refusing to write. This is almost certainly the wrong drive.",
                disk.id
            )));
        }
        Risk::Internal if !args.force => {
            return Err(ps5de_core::Error::Other(format!(
                "'{}' is an INTERNAL disk, not an external/USB drive. The PS5 drive is \
                 normally in a USB enclosure. If you are certain, re-run with --force.",
                disk.id
            )));
        }
        _ => {}
    }

    // Confirmation.
    if !args.yes {
        println!(
            "\nThis will ERASE the first 2 MB of the drive above.\n\
             Type the disk id ('{}') to continue, or anything else to abort:",
            disk.id
        );
        print!("> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|e| ps5de_core::Error::io("reading confirmation", e))?;
        if line.trim() != disk.id {
            return Err(ps5de_core::Error::Other("aborted by user".into()));
        }
    }

    println!();
    let mut bar = ProgressBar::new();
    ps5de_core::flash(&disk, &image, &mut |ev| bar.handle(ev))?;
    bar.finish();
    println!("Enabler image written.");

    if !args.no_verify {
        let mut bar = ProgressBar::new();
        ps5de_core::verify(&disk, &image, &mut |ev| bar.handle(ev))?;
        bar.finish();
        println!("Verification passed.");
    }

    println!(
        "\nDone. Next steps:\n\
         \x20 1. Safely eject the drive and remove it from the enclosure.\n\
         \x20 2. Install it into the PS5's M.2 expansion slot.\n\
         \x20 3. Power on. The SSD shows up under Settings > Storage, ready to use.\n"
    );
    Ok(())
}

fn cmd_verify(args: VerifyArgs) -> ps5de_core::Result<()> {
    let disk = ps5de_core::find_disk(&args.device)?;
    let image = match &args.image {
        Some(p) => ps5de_core::image::load_from_path(p)?,
        None => ps5de_core::embedded_image().to_vec(),
    };
    print_target(&disk);
    println!();
    let mut bar = ProgressBar::new();
    ps5de_core::verify(&disk, &image, &mut |ev| bar.handle(ev))?;
    bar.finish();
    println!("Verification passed: the enabler image is present on this drive.");
    Ok(())
}

fn print_target(d: &Disk) {
    println!("\nTarget drive:");
    println!("  ID:    {}", d.id);
    println!("  Model: {}", if d.model.is_empty() { "(unknown)" } else { &d.model });
    println!("  Size:  {}", d.size_pretty());
    println!("  Bus:   {}", d.bus);
    println!("  Type:  {}", risk_label(d.risk()));
}

fn risk_label(r: Risk) -> &'static str {
    match r {
        Risk::System => "SYSTEM  <-- do NOT use",
        Risk::Internal => "internal disk",
        Risk::Removable => "external/USB (expected)",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// Minimal single-line progress bar rendered to stderr.
struct ProgressBar {
    stage: String,
    last_pct: i32,
}

impl ProgressBar {
    fn new() -> Self {
        ProgressBar {
            stage: String::new(),
            last_pct: -1,
        }
    }

    fn handle(&mut self, ev: ProgressEvent) {
        match ev {
            ProgressEvent::Stage(s) => {
                if !self.stage.is_empty() {
                    eprintln!();
                }
                self.stage = s.to_string();
                self.last_pct = -1;
                eprint!("{s} ... ");
                io::stderr().flush().ok();
            }
            ProgressEvent::Progress { done, total } => {
                let pct = (done * 100).checked_div(total).unwrap_or(100) as i32;
                if pct != self.last_pct {
                    self.last_pct = pct;
                    let filled = (pct / 5) as usize;
                    eprint!(
                        "\r{} [{}{}] {pct:>3}%",
                        self.stage,
                        "#".repeat(filled),
                        " ".repeat(20 - filled),
                    );
                    io::stderr().flush().ok();
                }
            }
        }
    }

    fn finish(&mut self) {
        eprintln!();
    }
}
