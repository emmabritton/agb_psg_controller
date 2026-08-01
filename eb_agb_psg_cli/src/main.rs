mod apu;
mod play;
mod save;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use eb_agb_psg_controller::Sfx;
use eb_agb_psg_interop::emit::sfx_to_file;
use eb_agb_psg_interop::parse::parse_sfx;

use save::{Metadata, SaveKind, SavePayload};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    List {
        sav: PathBuf,
    },
    View {
        sav: PathBuf,
        #[arg(value_parser = clap::value_parser!(u8).range(..save::SAVE_SLOTS as i64))]
        slot: u8,
    },
    Extract {
        sav: PathBuf,
        #[arg(value_parser = clap::value_parser!(u8).range(..save::SAVE_SLOTS as i64))]
        slot: u8,
        #[arg(short, long)]
        out: Option<PathBuf>,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        wav: bool,
    },
    Insert {
        sav: PathBuf,
        #[arg(value_parser = clap::value_parser!(u8).range(..save::SAVE_SLOTS as i64))]
        slot: u8,
        psfx: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        init: bool,
    },
    Delete {
        sav: PathBuf,
        #[arg(value_parser = clap::value_parser!(u8).range(..save::SAVE_SLOTS as i64))]
        slot: u8,
    },
    Play {
        source: PathBuf,
        #[arg(value_parser = clap::value_parser!(u8).range(..save::SAVE_SLOTS as i64))]
        slot: Option<u8>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::List { sav } => list(&sav),
        Command::View { sav, slot } => {
            let (name, sfx) = read_slot_sfx(&sav, slot)?;
            let ron = sfx_to_file(&sfx)
                .and_then(|f| {
                    f.to_ron()
                        .map_err(|e| eb_agb_psg_interop::parse::ParseError {
                            message: e.to_string(),
                        })
                })
                .map_err(|e| format!("slot {slot}: {e}"))?;
            println!("// slot {slot}: \"{name}\"");
            println!("{ron}");
            Ok(())
        }
        Command::Extract {
            sav,
            slot,
            out,
            force,
            wav,
        } => extract(&sav, slot, out, force, wav),
        Command::Insert {
            sav,
            slot,
            psfx,
            name,
            force,
            init,
        } => insert(&sav, slot, &psfx, name, force, init),
        Command::Delete { sav, slot } => {
            let (mut manager, bytes) = save::open_write(&sav)?;
            manager
                .erase(slot as usize)
                .map_err(|e| format!("erase failed: {e:?}"))?;
            save::persist(&sav, &bytes)?;
            println!("slot {slot} emptied");
            Ok(())
        }
        Command::Play { source, slot } => {
            let sfx = if source.extension().is_some_and(|e| e == "psfx") {
                let text = std::fs::read_to_string(&source)
                    .map_err(|e| format!("{}: {e}", source.display()))?;
                parse_sfx(&text).map_err(|e| format!("{}: {e}", source.display()))?
            } else {
                let slot = slot.ok_or("playing from a .sav needs a slot number")?;
                read_slot_sfx(&source, slot)?.1
            };
            play::run(&sfx)
        }
    }
}

fn slot_name(metadata: &Metadata) -> String {
    String::from_utf8_lossy(&metadata.name).into_owned()
}

fn list(sav: &Path) -> Result<(), String> {
    let manager = save::open_read(sav)?;
    println!("slot  status   kind   name                  instr  rows");
    for i in 0..manager.num_slots() {
        match manager.slot(i) {
            agb_save::Slot::Empty => println!("{i:>4}  empty"),
            agb_save::Slot::Corrupted => println!("{i:>4}  corrupt"),
            agb_save::Slot::Valid(meta) => {
                let kind = match meta.kind {
                    SaveKind::Sfx => "sfx",
                    SaveKind::Track => "track",
                };
                println!(
                    "{i:>4}  ok       {kind:<5}  {:<20}  {:>5}  {:>4}",
                    slot_name(meta),
                    meta.instruments,
                    meta.rows
                );
            }
        }
    }
    Ok(())
}

fn read_slot_sfx(sav: &Path, slot: u8) -> Result<(String, Sfx), String> {
    let mut manager = save::open_read(sav)?;
    let name = match manager.slot(slot as usize) {
        agb_save::Slot::Empty => return Err(format!("slot {slot} is empty")),
        agb_save::Slot::Corrupted => return Err(format!("slot {slot} is corrupted")),
        agb_save::Slot::Valid(meta) => slot_name(meta),
    };
    match manager
        .read::<SavePayload>(slot as usize)
        .map_err(|e| format!("slot {slot}: read failed: {e:?}"))?
    {
        SavePayload::Sfx(sfx) => Ok((name, sfx)),
        SavePayload::Track(_) => Err(format!("slot {slot} holds a track; only SFX are supported")),
    }
}

fn extract(
    sav: &Path,
    slot: u8,
    out: Option<PathBuf>,
    force: bool,
    wav: bool,
) -> Result<(), String> {
    let (name, sfx) = read_slot_sfx(sav, slot)?;
    let extension = if wav { "wav" } else { "psfx" };
    let out = out.unwrap_or_else(|| {
        let stem: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        let stem = stem.trim_matches('_');
        PathBuf::from(if stem.is_empty() {
            format!("slot{slot}.{extension}")
        } else {
            format!("{stem}.{extension}")
        })
    });
    if out.exists() && !force {
        return Err(format!("{} exists (use --force)", out.display()));
    }
    if wav {
        let seconds = play::render_wav(&sfx, &out)?;
        println!(
            "slot {slot} (\"{name}\") -> {} ({seconds:.2}s)",
            out.display()
        );
    } else {
        let ron = sfx_to_file(&sfx)
            .map_err(|e| format!("slot {slot}: {e}"))?
            .to_ron()
            .map_err(|e| e.to_string())?;
        std::fs::write(&out, ron).map_err(|e| format!("{}: {e}", out.display()))?;
        println!("slot {slot} (\"{name}\") -> {}", out.display());
    }
    Ok(())
}

fn insert(
    sav: &Path,
    slot: u8,
    psfx: &Path,
    name: Option<String>,
    force: bool,
    init: bool,
) -> Result<(), String> {
    let text = std::fs::read_to_string(psfx).map_err(|e| format!("{}: {e}", psfx.display()))?;
    let sfx = parse_sfx(&text).map_err(|e| format!("{}: {e}", psfx.display()))?;

    let name = name.unwrap_or_else(|| {
        psfx.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    if name.is_empty() || name.len() > save::MAX_FILENAME_LEN || !name.is_ascii() {
        return Err(format!(
            "name \"{name}\" must be 1-{} ASCII characters (set one with --name)",
            save::MAX_FILENAME_LEN
        ));
    }

    let (mut manager, bytes) = if init {
        save::create_new()?
    } else {
        save::open_write(sav)?
    };
    if !init && !matches!(manager.slot(slot as usize), agb_save::Slot::Empty) && !force {
        return Err(format!("slot {slot} is not empty (use --force)"));
    }

    let metadata = Metadata {
        kind: SaveKind::Sfx,
        name: name.clone().into_bytes(),
        instruments: sfx.instruments.len().min(255) as u8,
        rows: sfx.rows.len().min(255) as u8,
    };
    manager
        .write(slot as usize, &SavePayload::Sfx(sfx), &metadata)
        .map_err(|e| format!("write failed: {e:?}"))?;
    save::persist(sav, &bytes)?;
    println!("{} -> slot {slot} (\"{name}\")", psfx.display());
    Ok(())
}
