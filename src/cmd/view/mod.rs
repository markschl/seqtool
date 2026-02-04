use crate::cli::Report;
use crate::config::Config;
use crate::error::CliResult;

mod cli;
mod color;
mod fmt;
mod pager;
mod pal;

pub use self::cli::*;
use self::color::*;
use self::fmt::*;
use self::pager::*;
use self::pal::*;

pub fn run(mut cfg: Config, args: ViewCommand) -> CliResult<Option<Box<dyn Report>>> {
    if args.color.list_pal {
        print_palettes(
            &args.color.textcols,
            args.color.truecolor.unwrap_or_else(has_truecolor),
        )?;
        return Ok(None);
    }

    // setup colors
    use ColorSource::*;
    let use_qual = cfg.input_config[0].format.format.has_qual() && !args.general.no_qual;
    let (bg, fg, bold) = if use_qual {
        if args.general.foreground {
            (Some(Qual), Some(Seq), true)
        } else {
            (Some(Qual), None, false)
        }
    } else if args.general.foreground {
        (None, Some(Seq), false)
    } else {
        (Some(Seq), None, false)
    };
    let mut formatter = Formatter::new(args.general.id_len, args.general.show_desc)
        .capabilities(
            args.color.truecolor.unwrap_or_else(has_truecolor),
            has_utf8(),
        )
        .textcols(args.color.textcols.0, args.color.textcols.1)?
        .color_config(bg, fg)
        .bold(args.general.bold || bold);

    let palettes = args.color.palettes();
    let mut pager = GrowingPager::new();
    let mut terminal = ratatui::init();
    // let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))?;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Progress {
        New,
        Running,
        Done,
    }

    let mut progress = Progress::New;
    let res = cfg.read(|record, ctx| {
        loop {
            match pager.check_draw(&mut terminal, false)? {
                Status::Ok => {}
                Status::MissingLines => {
                    let (line, len) =
                        formatter.format(record, &mut ctx.qual_converter, &palettes)?;
                    progress = Progress::Running;
                    if use_qual {
                        pager.set_color_scale(formatter.format_scale(0, (2..47).step_by(2)));
                    }
                    pager.add(line, len);
                    break;
                }
                Status::Quit => {
                    progress = Progress::Done;
                    return Ok(false);
                }
            }
        }
        Ok(true)
    });
    if res.is_ok() && progress == Progress::Running {
        while !matches!(pager.check_draw(&mut terminal, true)?, Status::Quit) {}
    }
    ratatui::restore();
    res.map(|_| None)
}
