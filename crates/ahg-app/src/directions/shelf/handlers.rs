//! What a press on a Shelf control calls, for the two looks that wear it (Shelf and The
//! collection). Routing only: each handler is one action of `crate::actions`.
use rune::Tx;

use crate::actions;
use crate::gate::{Choice, State};
use crate::keys::RowAct;

/// A row's action button: the action its key names, on this task.
pub(crate) fn row_act(act: RowAct, id: &str) -> impl Fn(&mut State, &mut Tx) + 'static {
    let id = id.to_owned();
    move |s, tx| {
        let run: fn(&mut State, &mut Tx, &str) = match act {
            RowAct::Promote => actions::promote,
            RowAct::Pause => actions::pause,
            RowAct::Resume => actions::resume,
            RowAct::Cancel => actions::cancel,
            RowAct::CopyDetails => actions::copy_details,
            RowAct::Fix => actions::fix,
            RowAct::DownloadAgain => actions::download_again,
            RowAct::Retry => actions::retry,
            RowAct::OpenFile => actions::open_file,
            RowAct::ShowInFolder => actions::show_in_folder,
            RowAct::Remove => actions::remove,
        };
        run(s, tx, &id)
    }
}

/// The found step's choice with one part changed.
pub(crate) fn choose(edit: impl Fn(&mut Choice) + 'static) -> impl Fn(&mut State) + 'static {
    move |s| {
        let mut c = s.choice.clone();
        edit(&mut c);
        actions::set_choice(s, c)
    }
}
