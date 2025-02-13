use std::thread::sleep;
use std::time::Duration;

fn main() {
    
    
    let mut m = machine_info::Machine::new();
    loop {
        let info = m.system_info();
        let stat = m.system_status();

        println!("sys: {info:#?}\nstat:{stat:#?}");
        sleep(Duration::from_secs(10));
    }
}
