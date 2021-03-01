#![no_std]
use libtock::println;
/**
 * This example shows a repeated timer combined with reading and displaying the current time in
 * clock ticks.
 **/
use libtock::ble_composer;
use libtock::ble_composer::BlePayload;
use libtock::ble_parser;
use core::cell::Cell;
use libtock::buttons::ButtonState;
use libtock::hmac::{HmacDataBuffer, HmacDestBuffer, HmacKeyBuffer};
use libtock::result::TockResult;
use libtock::simple_ble;
use libtock::simple_ble::BleAdvertisingDriver;
use serde::Serialize;
use futures::future;
use libtock::timer::DriverContext;
use libtock::timer::Duration;
use libtock::syscalls;

libtock_core::stack_size! {0x800}

const DELAY_MS: usize = 500;

#[derive(Deserialize)]
struct LedCommand {
    pub nr: u8,
    pub st: bool,
}


#[libtock::main]
async fn main() -> TockResult<()> {
    let mut drivers = libtock::retrieve_drivers()?;
    let hmac_driver = drivers.hmac.init_driver()?;
    let buttons_driver = drivers.buttons.init_driver()?;
    let mut ble_advertising_driver = drivers.ble_advertising.create_driver();
    let mut ble_scanning_driver_factory = drivers.ble_scanning;
    let mut ble_scanning_driver = ble_scanning_driver_factory.create_driver();
    let mut ble_scanning_driver_sharing = ble_scanning_driver.share_memory()?;
    let ble_scanning_driver_scanning = ble_scanning_driver_sharing.start()?;
    let pressed_count = Cell::new(0usize);
    let released_count = Cell::new(0usize);

    let mut callback = |_button_num, state| match state {
        ButtonState::Pressed => pressed_count.set(pressed_count.get() + 1),
        ButtonState::Released => released_count.set(released_count.get() + 1),
    };

    let _subscription = buttons_driver.subscribe(&mut callback)?;

    for button in buttons_driver.buttons() {
        button.enable_interrupt()?;
    }

    drivers.console.create_console();
    
    println!("Loading in 0 key");
    let mut key_buffer = HmacKeyBuffer::default();
    let _key_buffer = hmac_driver.init_key_buffer(&mut key_buffer)?;
    println!("  done");

    println!("Creating data buffer");
    let mut data_buffer = HmacDataBuffer::default();
    let data: &[u8; 72] =
        b"A language empowering everyone to build reliable and efficient software.";

    for (i, d) in data.iter().enumerate() {
        data_buffer[i] = *d;
    }
    let _data_buffer = hmac_driver.init_data_buffer(&mut data_buffer)?;
    println!("  done");

    println!("Creating dest buffer");
    let mut dest_buffer = HmacDestBuffer::default();
    let dest_buffer = hmac_driver.init_dest_buffer(&mut dest_buffer)?;
    println!("  done");

    let mut temp_buffer = [0; libtock::hmac::DEST_BUFFER_SIZE];

    println!("Setting callback and running");
    let mut callback = |_result, _digest| {
        println!("HMAC Complete, printing digest");
        dest_buffer.read_bytes(&mut temp_buffer[..]);

        for buf in temp_buffer.iter().take(libtock::hmac::DEST_BUFFER_SIZE) {
            println!("{:x}", *buf);
        }
    };

    let _subscription = hmac_driver.subscribe(&mut callback)?;

    hmac_driver.run()?;

    let uuid: [u8; 2] = [0x00, 0x18];

    let payload = corepack::to_bytes(LedCommand { nr: 2, st: true }).unwrap();

    let mut buffer = BleAdvertisingDriver::create_advertising_buffer();
    let mut gap_payload = BlePayload::default();

    gap_payload
        .add_flag(ble_composer::flags::LE_GENERAL_DISCOVERABLE)
        .unwrap();

    gap_payload
        .add(ble_composer::gap_types::UUID, &uuid)
        .unwrap();

    gap_payload
        .add(ble_composer::gap_types::COMPLETE_LOCAL_NAME, b"Tock!")
        .unwrap();

    gap_payload.add_service_payload([91, 79], &payload).unwrap();

    let _handle = ble_advertising_driver.initialize(100, &gap_payload, &mut buffer);

    
    let mut previous_ticks = None;

    for i in 0.. {
        print_now(&mut drivers.timer, &mut previous_ticks, i)?;
        let mut timer_driver = drivers.timer.create_timer_driver();
        let timer_driver = timer_driver.activate()?;

        timer_driver.sleep(Duration::from_ms(DELAY_MS)).await?;
    }
    
    let mut with_callback = drivers.timer.with_callback(|_, _| {
        println!("This line is printed 2 seconds after the start of the program.");
    });

    let mut timer = with_callback.init()?;
    timer.set_alarm(Duration::from_ms(2000))?;

    future::pending().await
    Ok(())
    loop {
        unsafe { syscalls::raw::yieldk() };
    }
}
fn connecting(){
      let value = ble_scanning_driver_scanning.stream_values().await;
        ble_parser::find(&value, simple_ble::gap_data::SERVICE_DATA as u8)
            .and_then(|service_data| ble_parser::extract_for_service([91, 79], service_data))
            .and_then(|payload| corepack::from_bytes::<LedCommand>(&payload).ok())
            .and_then(|msg| leds_driver.get(msg.nr as usize).ok())
            .and_then(|led| led.on().ok());
}
fn print_now(
    timer_context: &mut DriverContext,
    previous_ticks: &mut Option<isize>,
    i: usize,
) -> TockResult<()> {
    let mut timer_with_callback = timer_context.with_callback(|_, _| {});
    let timer = timer_with_callback.init()?;
    let current_clock = timer.get_current_clock()?;
    let ticks = current_clock.num_ticks();
    let frequency = timer.clock_frequency().hz();
    println!(
        "[{}] Waited roughly {}. Now is {} = {:#010x} ticks ({:?} ticks since last time at {} Hz)",
        i,
        PrettyTime::from_ms(i * DELAY_MS),
        PrettyTime::from_ms(current_clock.ms_f64() as usize),
        ticks,
        previous_ticks.map(|previous| ticks - previous),
        frequency
    );
    *previous_ticks = Some(ticks);
    Ok(())
}

struct PrettyTime {
    mins: usize,
    secs: usize,
    ms: usize,
}

impl PrettyTime {
    fn from_ms(ms: usize) -> PrettyTime {
        PrettyTime {
            ms: ms % 1000,
            secs: (ms / 1000) % 60,
            mins: ms / (60 * 1000),
        }
    }
}

impl core::fmt::Display for PrettyTime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.mins != 0 {
            write!(f, "{}m", self.mins)?
        }
        write!(f, "{}.{:03}s", self.secs, self.ms)
    }
}
