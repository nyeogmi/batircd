use std::{borrow::Borrow, time::Duration};

use tokio::time::Instant;

use crate::{sock::{MessageIn, MessageOut}, protocol::{Command, IRCString}};

pub fn parse(msg: &MessageIn) -> Option<Command> {
    let mut data: &[u8] = &msg.data.bytes;
    let pfx = if data.starts_with(b":") {
        let pfx_start = 1;
        let mut pfx_end = 1;
        while pfx_end < data.len() && data[pfx_end] != b' ' {
            pfx_end += 1;
        }

        let p = &data[pfx_start..pfx_end];
        data = &data[(pfx_end+1).min(data.len())..];
        Some(IRCString::new(p.to_vec()))
    } else {
        None
    };

    let mut args = split_args(&data);
    if args.len() == 0 { return None }
    let mut cmd = args.remove(0);
    cmd.upper_inplace();

    Some(Command { 
        pfx,
        cmd,
        args,
    })
}

// TODO: Smallvec
fn split_args(mut src: &[u8]) -> Vec<IRCString> {
    let mut out = vec![];

    loop {
        let arg_start = 0;
        let mut arg_end = 0;

        if arg_start >= src.len() { break; }

        if src[arg_start] == b':' {
            out.push(IRCString::new((&src[arg_start + 1..]).to_vec()));
            break;
        }

        while arg_end < src.len() && src[arg_end] != b' ' {
            arg_end += 1;
        }
        out.push(IRCString::new((&src[arg_start..arg_end]).to_vec()));

        src = &src[(arg_end+1).min(src.len())..]
    };

    return out
}

pub fn dump(command: Command, deadline_seconds: f32) -> MessageOut {
    // TODO: Break up long messages
    let mut out = vec![];
    if let Some(pfx) = command.pfx {
        out.push(b':');
        out.extend(pfx.bytes);
        out.push(b' ');
    }
    out.extend(command.cmd.bytes);
    for a in command.args {
        out.push(b' ');
        out.extend(a.bytes);
    }
    out.extend(b"\r\n");
    MessageOut { 
        deadline: Instant::now().checked_add(Duration::from_secs_f32(deadline_seconds)).unwrap(), 
        data: IRCString::new(out)
    }
}