use matches::*;
use std::char;
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter, Read, Write};
use std::iter::Peekable;
use std::path::PathBuf;
use std::str::FromStr;
use std::{u16, u32, u8};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt()]
struct Opt {
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: Option<PathBuf>,

    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
}

fn main() {
    let opt = Opt::from_args();

    let stdin = io::stdin();
    let mut input = opt
        .input
        .as_ref()
        .map(|path| File::open(path))
        .ok_or_else(|| stdin.lock());
    let mut input = match input {
        Ok(Ok(ref mut file)) => BufReader::<&mut Read>::new(file),
        Err(ref mut stdin) => BufReader::<&mut Read>::new(stdin),
        Ok(Err(err)) => {
            eprintln!("{}: {}", opt.input.unwrap().to_string_lossy(), err);
            return;
        }
    };

    let stdout = io::stdout();
    let mut output = opt
        .output
        .as_ref()
        .map(|path| File::create(path))
        .ok_or_else(|| stdout.lock());
    let mut output = match output {
        Ok(Ok(ref mut file)) => BufWriter::<&mut Write>::new(file),
        Err(ref mut stdout) => BufWriter::<&mut Write>::new(stdout),
        Ok(Err(err)) => {
            eprintln!("{}: {}", opt.output.unwrap().to_string_lossy(), err);
            return;
        }
    };

    let mut buf = String::new();
    if let Err(err) = input.read_to_string(&mut buf) {
        eprintln!("{}: {}", opt.input.unwrap().to_string_lossy(), err);
        return;
    };

    let mut input = buf.chars().peekable();
    skip_ws(&mut input);
    parse_value(&mut input, &mut output);
}

fn skip_ws(stdin: &mut Peekable<impl Iterator<Item = char>>) {
    while matches!(
        stdin.peek(),
        Some('\u{0020}') | Some('\u{0009}') | Some('\u{000a}') | Some('\u{000d}')
    ) {
        stdin.next();
    }
}

fn parse_value(stdin: &mut Peekable<impl Iterator<Item = char>>, stdout: &mut impl Write) {
    let c = stdin.peek().unwrap();
    assert_matches!(c, 'f' | 'n' | 't' | '{' | '[' | '-' | '0'..='9' | '"');
    match c {
        'f' => parse_false(stdin, stdout),
        'n' => parse_null(stdin, stdout),
        't' => parse_true(stdin, stdout),
        '{' => parse_object(stdin, stdout),
        '[' => parse_array(stdin, stdout),
        '-' | '0'..='9' => parse_number(stdin, stdout),
        '"' => parse_string(stdin, stdout),
        _ => unreachable!(),
    }
}

fn parse_false(stdin: &mut Peekable<impl Iterator<Item = char>>, stdout: &mut impl Write) {
    assert_eq!(stdin.by_ref().take(5).collect::<String>(), "false");
    stdout.write_all(b"\xc2").unwrap();
}

fn parse_null(stdin: &mut Peekable<impl Iterator<Item = char>>, stdout: &mut impl Write) {
    assert_eq!(stdin.by_ref().take(4).collect::<String>(), "null");
    stdout.write_all(b"\xc0").unwrap();
}

fn parse_true(stdin: &mut Peekable<impl Iterator<Item = char>>, stdout: &mut impl Write) {
    assert_eq!(stdin.by_ref().take(4).collect::<String>(), "true");
    stdout.write_all(b"\xc3").unwrap();
}

fn parse_object(stdin: &mut Peekable<impl Iterator<Item = char>>, stdout: &mut impl Write) {
    skip_ws(stdin);
    assert_eq!(stdin.next(), Some('{'));
    skip_ws(stdin);

    let mut buf = vec![0u8; 5];
    let mut n = 0usize;
    while stdin.peek().unwrap() != &'}' {
        parse_member(stdin, &mut buf);
        n += 1;

        skip_ws(stdin);
        if stdin.peek().unwrap() == &',' {
            stdin.next();
            skip_ws(stdin);
        }
    }

    skip_ws(stdin);
    assert_eq!(stdin.next(), Some('}'));
    skip_ws(stdin);

    if n <= 0b1111usize {
        buf[4] = 0b1000_0000u8 | (n as u8);
        stdout.write_all(&buf[4..]).unwrap();
    } else if n <= u16::MAX as usize {
        let n = n as u16;
        buf[2] = 0xde_u8;
        buf[3] = (n >> 8) as u8;
        buf[4] = (0x00ff_u16 & n) as u8;
        stdout.write_all(&buf[2..]).unwrap();
    } else if n <= u32::MAX as usize {
        let n = n as u32;
        buf[0] = 0xdf_u8;
        buf[1] = (n >> 24) as u8;
        buf[2] = (0x0000_00ff_u32 & (n >> 16)) as u8;
        buf[3] = (0x0000_00ff_u32 & (n >> 8)) as u8;
        buf[4] = (0x0000_00ff_u32 & n) as u8;
        stdout.write_all(&buf).unwrap();
    } else {
        panic!();
    }
}

fn parse_member(stdin: &mut Peekable<impl Iterator<Item = char>>, stdout: &mut impl Write) {
    parse_string(stdin, stdout);
    skip_ws(stdin);
    assert_eq!(stdin.next(), Some(':'));
    skip_ws(stdin);
    parse_value(stdin, stdout);
}

fn parse_array(stdin: &mut Peekable<impl Iterator<Item = char>>, stdout: &mut impl Write) {
    skip_ws(stdin);
    assert_eq!(stdin.next(), Some('['));
    skip_ws(stdin);

    let mut buf = vec![0u8; 5];
    let mut n = 0usize;
    while stdin.peek().unwrap() != &']' {
        parse_value(stdin, &mut buf);
        n += 1;

        skip_ws(stdin);
        if stdin.peek().unwrap() == &',' {
            stdin.next();
            skip_ws(stdin);
        }
    }

    skip_ws(stdin);
    assert_eq!(stdin.next(), Some(']'));
    skip_ws(stdin);

    if n <= 0b1111usize {
        buf[4] = 0b1001_0000u8 | (n as u8);
        stdout.write_all(&buf[4..]).unwrap();
    } else if n <= u16::MAX as usize {
        let n = n as u16;
        buf[2] = 0xdc_u8;
        buf[3] = (n >> 8) as u8;
        buf[4] = (0x00ff_u16 & n) as u8;
        stdout.write_all(&buf[2..]).unwrap();
    } else if n <= u32::MAX as usize {
        let n = n as u32;
        buf[0] = 0xdd_u8;
        buf[1] = (n >> 24) as u8;
        buf[2] = (0x0000_00ffu32 & (n >> 16)) as u8;
        buf[3] = (0x0000_00ffu32 & (n >> 8)) as u8;
        buf[4] = (0x0000_00ffu32 & n) as u8;
        stdout.write_all(&buf).unwrap();
    } else {
        panic!();
    }
}

fn parse_number(stdin: &mut Peekable<impl Iterator<Item = char>>, stdout: &mut impl Write) {
    let mut number = String::new();
    while let Some(&c) = stdin.peek() {
        if "[{]}:,".contains(c) {
            break;
        }
        number.push(c);
        stdin.next();
    }

    match u8::from_str(&number) {
        Ok(number) if number & 0b0111_1111 == number => {
            stdout.write_all(&[number]).unwrap();
            return;
        }
        Ok(number) => {
            stdout.write_all(&[b'\xcc', number]).unwrap();
            return;
        }
        _ => {}
    }

    match i8::from_str(&number) {
        Ok(number) if number as u8 & 0b0001_1111 == number as u8 => {
            stdout.write_all(&[number as u8]).unwrap();
            return;
        }
        Ok(number) => {
            stdout.write_all(&[b'\xd0', number as u8]).unwrap();
            return;
        }
        _ => {}
    }

    if let Ok(number) = u16::from_str(&number) {
        stdout.write_all(b"\xcd").unwrap();
        stdout.write_all(&number.to_be_bytes()).unwrap();
    } else if let Ok(number) = u32::from_str(&number) {
        stdout.write_all(b"\xce").unwrap();
        stdout.write_all(&number.to_be_bytes()).unwrap();
    } else if let Ok(number) = u64::from_str(&number) {
        stdout.write_all(b"\xcf").unwrap();
        stdout.write_all(&number.to_be_bytes()).unwrap();
    } else if let Ok(number) = i16::from_str(&number) {
        stdout.write_all(b"\xd1").unwrap();
        stdout.write_all(&number.to_be_bytes()).unwrap();
    } else if let Ok(number) = i32::from_str(&number) {
        stdout.write_all(b"\xd2").unwrap();
        stdout.write_all(&number.to_be_bytes()).unwrap();
    } else if let Ok(number) = i64::from_str(&number) {
        stdout.write_all(b"\xd3").unwrap();
        stdout.write_all(&number.to_be_bytes()).unwrap();
    } else if let Ok(number) = f64::from_str(&number) {
        stdout.write_all(b"\xcb").unwrap();
        stdout.write_all(&number.to_bits().to_be_bytes()).unwrap();
    } else if let Ok(number) = f32::from_str(&number) {
        stdout.write_all(b"\xca").unwrap();
        stdout.write_all(&number.to_bits().to_be_bytes()).unwrap();
    } else if let Ok(number) = f64::from_str(&number) {
        stdout.write_all(b"\xcb").unwrap();
        stdout.write_all(&number.to_bits().to_be_bytes()).unwrap();
    }
}

fn parse_string(stdin: &mut Peekable<impl Iterator<Item = char>>, stdout: &mut impl Write) {
    assert_eq!(stdin.next(), Some('"'));
    let mut buf = String::from_utf8(vec![0, 0, 0, 0, 0]).unwrap();
    while let Some(c) = stdin.next() {
        match c {
            '"' => break,
            '\u{0020}' | '\u{0021}' | '\u{0023}'..='\u{005b}' | '\u{005d}'..='\u{10FFFF}' => {
                buf.push(c)
            }
            '\\' => {
                let c = stdin.next().unwrap();
                buf.push(match c {
                    '"' | '\\' | '/' => c,
                    'b' => '\u{0008}',
                    'f' => '\u{000c}',
                    'n' => '\u{000a}',
                    'r' => '\u{000d}',
                    't' => '\u{0009}',
                    'u' => char::from_u32(
                        u32::from_str_radix(&stdin.take(4).collect::<String>(), 16).unwrap(),
                    )
                    .unwrap(),
                    _ => panic!(),
                });
            }
            _ => panic!(),
        }
    }

    let mut buf = buf.into_bytes();
    let n = buf.len() - 5;
    if n <= 0b11111usize {
        buf[4] = 0b1010_0000u8 | (n as u8);
        stdout.write_all(&buf[4..]).unwrap();
    } else if n <= u8::MAX as usize {
        let n = n as u8;
        buf[3] = 0xd9_u8;
        buf[4] = n as u8;
        stdout.write_all(&buf[3..]).unwrap();
    } else if n <= u16::MAX as usize {
        let n = n as u16;
        buf[2] = 0xda_u8;
        buf[3] = (n >> 8) as u8;
        buf[4] = (0x00ff_u16 & n) as u8;
        stdout.write_all(&buf[2..]).unwrap();
    } else if n <= u32::MAX as usize {
        let n = n as u32;
        buf[0] = 0xdb_u8;
        buf[1] = (n >> 24) as u8;
        buf[2] = (0x0000_00ffu32 & (n >> 16)) as u8;
        buf[3] = (0x0000_00ffu32 & (n >> 8)) as u8;
        buf[4] = (0x0000_00ffu32 & n) as u8;
        stdout.write_all(&buf).unwrap();
    } else {
        panic!();
    }
}
