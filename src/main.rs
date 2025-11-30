use probe_rs::architecture::arm::component::Dwt;
use probe_rs::architecture::arm::component::Etm;
use probe_rs::architecture::arm::component::TraceSink;
use probe_rs::architecture::arm::component::find_component;
use probe_rs::architecture::arm::dp::DpAddress;
use probe_rs::architecture::arm::memory::PeripheralType;
use probe_rs::probe::list::Lister;
use probe_rs::{Permissions, flashing};

use std::path::Path;
use std::time::Duration;

// Arbitrary elf or ihex file.
const PATH: &str = "../dma_test/main.elf";

fn main() {
    // List all probes
    let lister = Lister::new();
    let list = lister.list_all();

    // Select the first probe
    let probe = if !list.is_empty() {
        &list[0]
    } else {
        panic!("no probe found");
    };

    let probe = probe.open().unwrap();

    let mut session = probe
        .attach("STM32H7B0VB", Permissions::new().allow_erase_all())
        .unwrap();

    let fmt = match Path::new(PATH)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("hex") | Some("ihex") => flashing::Format::Hex,
        Some("elf") | _ => {
            flashing::Format::Elf(flashing::ElfOptions::default())
        }
    };

    flashing::download_file(&mut session, PATH, fmt).unwrap();

    // Configure DWT
    let components = session.get_arm_components(DpAddress::Default).unwrap();
    {
        let intf = session.get_arm_interface().unwrap();
        let (intf, comp) = (
            intf,
            find_component(&components, PeripheralType::Dwt).unwrap(),
        );
        let mut dwt = Dwt::new(intf, comp);
        dwt.enable().unwrap();

        // This is the PC address that creates an CMPEVENT in unit 1;
        // This is used for start.
        dwt.enable_instruction_event(0, 0x80046b0).unwrap();
        // This is the PC address that creates an CMPEVENT in unit 2;
        // This is used for stop.
        dwt.enable_instruction_event(1, 0x80046c0).unwrap();
    }

    let _ = session.setup_tracing(0, TraceSink::TraceMemory);


    let mut decoder = {
        let intf = session.get_arm_interface().unwrap();
        let mut etm = Etm::load(intf, &components).unwrap();
        // Start and stop unit could be used by supplying a custom EtmV4Config.
        let _ = etm.enable_instruction_trace();
        etm.decoder().unwrap()
    };

    session.core(0).unwrap().reset().unwrap();

    // This is used to make sure the buffer is populated
    std::thread::sleep(Duration::from_secs(5));

    let trace_data = session.read_trace_data(Some(0x3E)).unwrap();

    println!("{:x?}", trace_data);

    println!("{:x?}", decoder.feed(&trace_data));
}
