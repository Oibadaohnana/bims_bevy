// Bit-compares libm results across CPU paths: run twice, diff the dumps.
use std::io::Write;
struct R(u64);
impl R { fn f(&mut self)->f64{ self.0^=self.0<<13; self.0^=self.0>>7; self.0^=self.0<<17; (self.0>>11) as f64/(1u64<<53) as f64 } }
fn main(){
    let n=4_000_000usize;
    let out=std::env::args().nth(1).unwrap();
    if out=="cmp" { let (a,b)=(std::env::args().nth(2).unwrap(),std::env::args().nth(3).unwrap());
        let mut names:Vec<_>=std::fs::read_dir(&a).unwrap().map(|e|e.unwrap().file_name().into_string().unwrap()).collect(); names.sort();
        for nm in names { let x=std::fs::read(format!("{a}/{nm}")).unwrap(); let y=std::fs::read(format!("{b}/{nm}")).unwrap();
            let d=x.chunks(8).zip(y.chunks(8)).filter(|(p,q)|p!=q).count(); println!("{nm:14} {d:8} of {} differ", x.len()/8); } return; }
    std::fs::create_dir_all(&out).unwrap();
    macro_rules! run32 { ($name:expr, $lo:expr, $hi:expr, |$a:ident,$b:ident| $e:expr) => {{
        let mut r=R(0x9E3779B97F4A7C15); let mut v=Vec::with_capacity(n);
        for _ in 0..n { let $a=($lo+($hi-$lo)*r.f()) as f32; let $b=($lo+($hi-$lo)*r.f()) as f32; let x:f32=$e; v.push(x.to_bits() as u64); }
        let mut f=std::fs::File::create(format!("{}/{}",out,$name)).unwrap(); for x in &v { f.write_all(&x.to_le_bytes()).unwrap(); } }}}
    macro_rules! run64 { ($name:expr, $lo:expr, $hi:expr, |$a:ident,$b:ident| $e:expr) => {{
        let mut r=R(0x9E3779B97F4A7C15); let mut v=Vec::with_capacity(n);
        for _ in 0..n { let $a=$lo+($hi-$lo)*r.f(); let $b=$lo+($hi-$lo)*r.f(); let x:f64=$e; v.push(x.to_bits()); }
        let mut f=std::fs::File::create(format!("{}/{}",out,$name)).unwrap(); for x in &v { f.write_all(&x.to_le_bytes()).unwrap(); } }}}
    let a=std::hint::black_box(1.0f32); let _=a;
    run32!("f32.sin", -20.0, 20.0, |a,_b| std::hint::black_box(a).sin());
    run32!("f32.cos", -20.0, 20.0, |a,_b| std::hint::black_box(a).cos());
    run32!("f32.sin_big", -1e5, 1e5, |a,_b| std::hint::black_box(a).sin());
    run32!("f32.atan2", -500.0, 500.0, |a,b| std::hint::black_box(a).atan2(b));
    run32!("f32.exp", -30.0, 5.0, |a,_b| std::hint::black_box(a).exp());
    run32!("f32.powf", 0.0, 4.0, |a,b| std::hint::black_box(a).powf(b));
    run32!("f32.ln", 0.0, 100.0, |a,_b| std::hint::black_box(a).ln());
    run32!("f32.acos", -1.0, 1.0, |a,_b| std::hint::black_box(a).acos());
    run32!("f32.hypot", -500.0, 500.0, |a,b| std::hint::black_box(a).hypot(b));
    run32!("f32.powi3", -50.0, 50.0, |a,_b| std::hint::black_box(a).powi(std::hint::black_box(3)));
    run32!("f32.sqrt", 0.0, 1e6, |a,_b| std::hint::black_box(a).sqrt());
    run64!("f64.sin", -20.0, 20.0, |a,_b| std::hint::black_box(a).sin());
    run64!("f64.cos", -20.0, 20.0, |a,_b| std::hint::black_box(a).cos());
    run64!("f64.atan2", -1e4, 1e4, |a,b| std::hint::black_box(a).atan2(b));
    run64!("f64.hypot", -1e4, 1e4, |a,b| std::hint::black_box(a).hypot(b));
    run64!("f64.powf", 0.0, 4.0, |a,b| std::hint::black_box(a).powf(b));
    run64!("f64.exp", -30.0, 5.0, |a,_b| std::hint::black_box(a).exp());
    run64!("f64.ln", 0.0, 100.0, |a,_b| std::hint::black_box(a).ln());
    run64!("f64.sqrt", 0.0, 1e6, |a,_b| std::hint::black_box(a).sqrt());
}
