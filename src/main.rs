use std::io::prelude::*;

fn read_all<S: AsRef<std::path::Path>>(path: S) -> String {
    let mut content = String::new();
    let mut f = std::fs::File::open(path).unwrap();
    f.read_to_string(&mut content).unwrap();
    content
}

fn main() {
    let fname = std::env::args().nth(1).unwrap_or(String::from("example"));
    let content = read_all(fname);

    let (p0_snd, p1_rcv) = std::sync::mpsc::channel();
    let (p1_snd, p0_rcv) = std::sync::mpsc::channel();

    let mut p0 = Program::new(lister(&content), 0, p0_rcv, p0_snd);
    let mut p1 = Program::new(lister(&content), 1, p1_rcv, p1_snd);

    loop {
        let before = p0.sends;
        p0.run_till_empty_queue();
        if p0.sends == before {
            break;
        }

        let before = p1.sends;
        p1.run_till_empty_queue();
        if p1.sends == before {
            break;
        }

    }

    println!("sends = {}, {}", p0.sends, p1.sends);
}

type RegVal = i64;

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
enum Ref {
    Reg(char),
    Val(RegVal)
}

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
enum Operation {
    Set(char, Ref),
    Add(char, Ref),
    Mul(char, Ref),
    Mod(char, Ref),
    Snd(Ref),
    Rcv(char),
    Jgz(Ref, Ref),
}

impl Operation {
    fn from_str(cmd: &str) -> Self {
        let tokens = cmd.splitn(3, ' ').collect::<Vec<_>>();
        match tokens[0] {
            "set" => Set(Self::reg(tokens[1]), Self::ref_val(tokens[2])),
            "add" => Add(Self::reg(tokens[1]), Self::ref_val(tokens[2])),
            "mul" => Mul(Self::reg(tokens[1]), Self::ref_val(tokens[2])),
            "mod" => Mod(Self::reg(tokens[1]), Self::ref_val(tokens[2])),
            "snd" => Snd(Self::ref_val(tokens[1])),
            "rcv" => Rcv(Self::reg(tokens[1])),
            "jgz" => Jgz(Self::ref_val(tokens[1]), Self::ref_val(tokens[2])),
            _ => unreachable!()
        }
    }

    fn reg(token: &str) -> char {
        token.chars().nth(0).unwrap()
    }

    fn ref_val(token: &str) -> Ref {
        match token.parse::<RegVal>() {
            Ok(v) => Ref::Val(v),
            _ => Ref::Reg(Self::reg(token))
        }
    }
}

use Operation::*;

fn lister<S: AsRef<str>>(lister: S) -> Vec<Operation> {
    lister.as_ref().lines().map(|l| l.trim())
        .map(Operation::from_str)
        .collect()
}

#[derive(Default, Debug)]
struct Registers(std::collections::HashMap<char, RegVal>);

impl Registers {
    fn _get(&mut self, r: char) -> RegVal {
        *self.0.entry(r).or_insert(0)
    }
    fn get(&mut self, r: Ref) -> RegVal {
        match r {
            Ref::Val(v) => v,
            Ref::Reg(r) => self._get(r)
        }
    }
    fn set(&mut self, r: char, v: Ref) {
        let v = self.get(v);
        self.0.insert(r, v);
    }
    fn add(&mut self, r: char, v: Ref) {
        *self.0.entry(r).or_insert(0) += self.get(v);
    }
    fn mul(&mut self, r: char, v: Ref) {
        *self.0.entry(r).or_insert(0) *= self.get(v);
    }
    fn module(&mut self, r: char, v: Ref) {
        *self.0.entry(r).or_insert(0) %= self.get(v);
    }
}

use std::sync::mpsc::{Receiver, Sender};

struct Program{
    code: Vec<Operation>,
    registers: Registers,
    input: Receiver<RegVal>,
    output: Sender<RegVal>,
    sends: usize,
    pc: RegVal,
}

impl Program {
    fn new(code: Vec<Operation>, id: RegVal,
           input: Receiver<RegVal>, output: Sender<RegVal>) -> Self {
        let mut registers  = Registers::default();
        registers.set('p', Ref::Val(id));
        Self {
            code,
            registers,
            input,
            output,
            sends: 0,
            pc: 0,
        }
    }

    fn run_till_empty_queue(&mut self) {
        loop {
            match self.code[self.pc as usize] {
                Set(reg, val) => { self.registers.set(reg, val); }
                Add(reg, val) => { self.registers.add(reg, val); }
                Mul(reg1, reg2) => { self.registers.mul(reg1, reg2) }
                Mod(reg, val) => { self.registers.module(reg, val) }
                Snd(rval) => {
                    self.sends += 1;
                    self.output.send(self.registers.get(rval)).unwrap();
                }
                Rcv(reg) => {
                    match self.input.try_recv() {
                        Ok(v) => self.registers.set(reg, Ref::Val(v)),
                        Err(_) => {
                            return
                        },
                    }
                }
                Jgz(rval, amount) => {
                    let v = self.registers.get(rval);
                    if v > 0 {
                        self.pc += self.registers.get(amount) - 1;
                    }
                },
            }
            self.pc += 1;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    static CODE: &'static str = "\
                        set a 1\n\
                        add a 2\n\
                        mul a a\n\
                        mod a 5\n\
                        snd a\n\
                        set a 0\n\
                        rcv a\n\
                        jgz a -1\n\
                        set a 1\n\
                        jgz a -2\
                        ";

    use Ref::*;

    #[test]
    fn parse_lister() {

        assert_eq!(
            vec![Set('a', Val(1)),
                 Add('a', Val(2)),
                 Mul('a', Reg('a')),
                 Mod('a', Val(5)),
                 Snd(Reg('a')),
                 Set('a', Val(0)),
                 Rcv('a'),
                 Jgz(Reg('a'), Val(-1)),
                 Set('a', Val(1)),
                 Jgz(Reg('a'), Val(-2))],
            lister(CODE)
        )
    }

    #[test]
    fn jgz_parse() {
        assert_eq!(Jgz(Val(1), Val(3_)), Operation::from_str("jgz 1 3"))
    }
}
