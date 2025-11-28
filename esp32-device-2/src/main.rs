use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::gpio::{PinDriver, OutputPin, InputPin};
use esp_idf_svc::hal::ledc::{LedcDriver, LedcTimerDriver, config::TimerConfig};
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration, QoS, Event, EventPayload};
use core::fmt::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Estructura para LEDs controlables
struct LedController<'a> {
    led1: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
    led2: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
    led3: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
    states: [bool; 3],
}

impl<'a> LedController<'a> {
    fn new(
        led1: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
        led2: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
        led3: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
    ) -> Self {
        LedController {
            led1,
            led2,
            led3,
            states: [false; 3],
        }
    }

    fn set_led(&mut self, led_id: u8, state: bool) -> Result<(), esp_idf_svc::sys::EspError> {
        if led_id < 1 || led_id > 3 {
            return Err(esp_idf_svc::sys::EspError::from_infallible::<{esp_idf_svc::sys::ESP_ERR_INVALID_ARG}>());
        }
        
        let index = (led_id - 1) as usize;
        self.states[index] = state;
        
        match led_id {
            1 => if state { self.led1.set_high() } else { self.led1.set_low() },
            2 => if state { self.led2.set_high() } else { self.led2.set_low() },
            3 => if state { self.led3.set_high() } else { self.led3.set_low() },
            _ => return Err(esp_idf_svc::sys::EspError::from_infallible::<{esp_idf_svc::sys::ESP_ERR_INVALID_ARG}>()),
        }
    }

    fn toggle_led(&mut self, led_id: u8) -> Result<bool, esp_idf_svc::sys::EspError> {
        if led_id < 1 || led_id > 3 {
            return Err(esp_idf_svc::sys::EspError::from_infallible::<{esp_idf_svc::sys::ESP_ERR_INVALID_ARG}>());
        }
        
        let index = (led_id - 1) as usize;
        let new_state = !self.states[index];
        self.set_led(led_id, new_state)?;
        Ok(new_state)
    }

    fn get_state(&self, led_id: u8) -> bool {
        if led_id < 1 || led_id > 3 {
            return false;
        }
        self.states[(led_id - 1) as usize]
    }

    fn turn_off_all(&mut self) {
        for i in 1..=3 {
            let _ = self.set_led(i, false);
        }
    }

    fn turn_on_all(&mut self) {
        for i in 1..=3 {
            let _ = self.set_led(i, true);
        }
    }
}

// Estructura para botones
struct ButtonController<'a> {
    button1: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    button2: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    last_states: [bool; 2],
}

impl<'a> ButtonController<'a> {
    fn new(
        button1: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
        button2: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    ) -> Self {
        ButtonController {
            button1,
            button2,
            last_states: [false; 2],
        }
    }
    
    fn check_buttons(&mut self) -> Option<u8> {
        let states = [
            self.button1.is_low(),
            self.button2.is_low(),
        ];
        
        for (i, (&current, &last)) in states.iter().zip(self.last_states.iter()).enumerate() {
            if current && !last { // Detecta flanco descendente
                self.last_states[i] = current;
                return Some(i as u8 + 1);
            }
            self.last_states[i] = current;
        }
        
        None
    }
}

// Estructura para buzzer
struct BuzzerController<'a> {
    pwm: LedcDriver<'a>,
}

impl<'a> BuzzerController<'a> {
    fn new(pwm: LedcDriver<'a>) -> Self {
        BuzzerController { pwm }
    }

    fn beep(&mut self, frequency: u32, duration_ms: u64) -> Result<(), esp_idf_svc::sys::EspError> {
        // Configurar frecuencia y duty cycle
        self.pwm.set_frequency(frequency)?;
        self.pwm.set_duty(512)?; // 50% duty cycle (máx 1023 para 10 bits)
        
        FreeRtos::delay_ms(duration_ms as u32);
        
        // Apagar buzzer
        self.pwm.set_duty(0)?;
        Ok(())
    }

    fn triple_beep(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        for _ in 0..3 {
            self.beep(1000, 200)?;
            FreeRtos::delay_ms(100);
        }
        Ok(())
    }

    fn startup_sound(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        self.beep(500, 100)?;
        FreeRtos::delay_ms(50);
        self.beep(750, 100)?;
        FreeRtos::delay_ms(50);
        self.beep(1000, 200)?;
        Ok(())
    }
}

// Helper para JSON
struct ArrayWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> ArrayWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        ArrayWriter { buf, pos: 0 }
    }
    
    fn pos(&self) -> usize {
        self.pos
    }
}

impl<'a> core::fmt::Write for ArrayWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let end = self.pos + bytes.len();
        if end > self.buf.len() {
            return Err(core::fmt::Error);
        }
        self.buf[self.pos..end].copy_from_slice(bytes);
        self.pos = end;
        Ok(())
    }
}

// Estructura para comandos recibidos
#[derive(Debug)]
struct Command {
    from: String,
    to: String,
    command: String,
    led_id: Option<u8>,
    duration: Option<u64>,
}

impl Command {
    fn from_json(json_str: &str) -> Option<Self> {
        // Parser JSON básico manual (sin serde para ahorrar memoria)
        if let (Some(from), Some(to), Some(command)) = (
            extract_json_string(json_str, "from"),
            extract_json_string(json_str, "to"),
            extract_json_string(json_str, "command"),
        ) {
            let led_id = extract_json_number(json_str, "led_id").map(|n| n as u8);
            let duration = extract_json_number(json_str, "duration").map(|n| n as u64);
            
            Some(Command {
                from,
                to,
                command,
                led_id,
                duration,
            })
        } else {
            None
        }
    }
}

// Helpers para parsing JSON manual
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let search = format!("\"{}\":", key);
    if let Some(start) = json.find(&search) {
        let after_colon = &json[start + search.len()..];
        if let Some(quote_start) = after_colon.find('\"') {
            let value_start = quote_start + 1;
            if let Some(quote_end) = after_colon[value_start..].find('\"') {
                return Some(after_colon[value_start..value_start + quote_end].to_string());
            }
        }
    }
    None
}

fn extract_json_number(json: &str, key: &str) -> Option<u32> {
    let search = format!("\"{}\":", key);
    if let Some(start) = json.find(&search) {
        let after_colon = &json[start + search.len()..];
        let mut number_str = String::new();
        for c in after_colon.chars() {
            if c.is_ascii_digit() {
                number_str.push(c);
            } else if !number_str.is_empty() {
                break;
            }
        }
        number_str.parse().ok()
    } else {
        None
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("🚀 ESP32 Device #2 - Actuator (LEDs + Buzzer + Buttons)");
    println!("📡 Conectando a WiFi y MQTT...");

    let p = Peripherals::take().unwrap();
    let s = EspSystemEventLoop::take().unwrap();
    let n = EspDefaultNvsPartition::take().unwrap();

    // Configurar WiFi
    let mut w = BlockingWifi::wrap(
        EspWifi::new(p.modem, s.clone(), Some(n.clone())).unwrap(),
        s,
    ).unwrap();

    w.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: "UTP".try_into().unwrap(),
        password: "tecnologica".try_into().unwrap(),
        ..Default::default()
    })).unwrap();

    w.start().unwrap();
    w.connect().unwrap();
    w.wait_netif_up().unwrap();
    println!("✅ WiFi conectado");

    // Configurar LEDs (GPIO 25, 26, 27)
    let led1 = PinDriver::output(p.pins.gpio25.downgrade_output()).unwrap();
    let led2 = PinDriver::output(p.pins.gpio26.downgrade_output()).unwrap();
    let led3 = PinDriver::output(p.pins.gpio27.downgrade_output()).unwrap();
    
    let mut led_controller = LedController::new(led1, led2, led3);
    println!("✅ LEDs configurados (GPIO25, 26, 27)");
    
    // Configurar botones (GPIO 18, 19)
    let button1 = PinDriver::input(p.pins.gpio18.downgrade_input()).unwrap();
    let button2 = PinDriver::input(p.pins.gpio19.downgrade_input()).unwrap();
    
    let mut button_controller = ButtonController::new(button1, button2);
    println!("✅ Botones configurados (GPIO18, 19)");

    // Configurar buzzer con PWM (GPIO 21)
    let timer_config = TimerConfig::new().frequency(1000.into());
    let timer = LedcTimerDriver::new(p.ledc.timer0, &timer_config).unwrap();
    let pwm = LedcDriver::new(
        p.ledc.channel0,
        timer,
        p.pins.gpio21,
    ).unwrap();
    
    let mut buzzer = BuzzerController::new(pwm);
    println!("✅ Buzzer configurado (GPIO21)");

    // Sonido de inicio
    let _ = buzzer.startup_sound();

    // Test de LEDs
    println!("🔄 Test de LEDs...");
    for i in 1..=3 {
        let _ = led_controller.set_led(i, true);
        FreeRtos::delay_ms(200);
        let _ = led_controller.set_led(i, false);
        FreeRtos::delay_ms(200);
    }

    // Configurar MQTT
    let mqtt_conf = MqttClientConfiguration {
        keep_alive_interval: Some(core::time::Duration::from_secs(30)),
        ..Default::default()
    };

    let (mqtt, mut conn) = EspMqttClient::new(
        "mqtt://broker.hivemq.com:1883",
        &mqtt_conf,
    ).unwrap();

    // Suscribirse a comandos
    mqtt.subscribe("esp32/commands", QoS::AtLeastOnce).unwrap();
    println!("✅ Suscrito a esp32/commands");

    // Variables compartidas para comunicación entre threads
    let led_states = Arc::new(Mutex::new([false; 3]));
    let command_queue = Arc::new(Mutex::new(Vec::<Command>::new()));
    
    // Thread para manejar MQTT
    let mqtt_clone = mqtt.clone();
    let led_states_clone = led_states.clone();
    let command_queue_clone = command_queue.clone();
    
    thread::spawn(move || {
        println!("🔄 Iniciando thread MQTT...");
        loop {
            match conn.next() {
                Ok(Event::Received(msg)) => {
                    if let Ok(payload) = std::str::from_utf8(&msg.payload) {
                        println!("📨 Comando recibido: {}", payload);
                        
                        if let Some(command) = Command::from_json(payload) {
                            if command.to == "esp32-actuator-01" {
                                let mut queue = command_queue_clone.lock().unwrap();
                                queue.push(command);
                            }
                        }
                    }
                },
                Ok(_) => {},
                Err(e) => {
                    eprintln!("❌ Error MQTT: {:?}", e);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    });

    FreeRtos::delay_ms(1000);
    println!("✅ MQTT conectado y suscrito");
    println!("🎯 Sistema listo - esperando comandos y botones");

    // Loop principal
    loop {
        // 1. Procesar comandos recibidos
        let commands_to_process: Vec<Command> = {
            let mut queue = command_queue.lock().unwrap();
            let commands = queue.clone();
            queue.clear();
            commands
        };

        for command in commands_to_process {
            println!("⚡ Ejecutando comando: {} de {}", command.command, command.from);
            
            match command.command.as_str() {
                "LED_ON" => {
                    if let Some(led_id) = command.led_id {
                        let _ = led_controller.set_led(led_id, true);
                        println!("💡 LED {} encendido", led_id);
                    }
                },
                "LED_OFF" => {
                    if let Some(led_id) = command.led_id {
                        let _ = led_controller.set_led(led_id, false);
                        println!("💡 LED {} apagado", led_id);
                    }
                },
                "LED_TOGGLE" => {
                    if let Some(led_id) = command.led_id {
                        if let Ok(new_state) = led_controller.toggle_led(led_id) {
                            println!("🔄 LED {} {}", led_id, if new_state { "encendido" } else { "apagado" });
                        }
                    }
                },
                "LED_ALL_ON" => {
                    led_controller.turn_on_all();
                    println!("💡 Todos los LEDs encendidos");
                },
                "LED_ALL_OFF" => {
                    led_controller.turn_off_all();
                    println!("💡 Todos los LEDs apagados");
                },
                "BUZZER" => {
                    let duration = command.duration.unwrap_or(500);
                    let _ = buzzer.beep(1000, duration);
                    println!("🔊 Buzzer activado por {}ms", duration);
                },
                "BUZZER_TRIPLE" => {
                    let _ = buzzer.triple_beep();
                    println!("🔊 Triple beep ejecutado");
                },
                _ => {
                    println!("❓ Comando desconocido: {}", command.command);
                }
            }
        }

        // 2. Verificar botones locales
        if let Some(button_id) = button_controller.check_buttons() {
            println!("🔘 Botón {} presionado!", button_id);
            
            // Botón 1: Toggle LED 1
            if button_id == 1 {
                let _ = led_controller.toggle_led(1);
                println!("🔄 LED 1 toggleado por botón local");
            }
            
            // Botón 2: Activar buzzer y enviar comando a ESP32 #1
            if button_id == 2 {
                let _ = buzzer.beep(750, 300);
                
                // Enviar comando de vuelta a ESP32 #1
                let mut msg_buf = [0u8; 128];
                let msg_len = {
                    let mut cursor = ArrayWriter::new(&mut msg_buf);
                    write!(
                        cursor,
                        r#"{{"from":"esp32-actuator-01","to":"esp32-sensor-01","command":"ACKNOWLEDGE","timestamp":{}}}"#,
                        esp_idf_svc::sys::esp_timer_get_time() / 1000
                    ).unwrap();
                    cursor.pos()
                };

                let _ = mqtt.publish(
                    "esp32/commands",
                    QoS::AtLeastOnce,
                    false,
                    &msg_buf[..msg_len],
                );
                
                println!("🔊 Buzzer + comando ACKNOWLEDGE enviado");
            }
            
            // Publicar evento de botón
            let mut event_buf = [0u8; 128];
            let event_len = {
                let mut cursor = ArrayWriter::new(&mut event_buf);
                write!(
                    cursor,
                    r#"{{"device":"esp32-actuator-01","button_id":{},"action":"pressed","timestamp":{}}}"#,
                    button_id,
                    esp_idf_svc::sys::esp_timer_get_time() / 1000
                ).unwrap();
                cursor.pos()
            };

            let _ = mqtt.publish(
                "esp32/button/events",
                QoS::AtLeastOnce,
                false,
                &event_buf[..event_len],
            );
            
            FreeRtos::delay_ms(300); // Debounce
        }

        // 3. Enviar estado de LEDs periódicamente
        static mut LAST_STATUS_TIME: u64 = 0;
        let current_time = esp_idf_svc::sys::esp_timer_get_time() / 1000;
        
        unsafe {
            if current_time - LAST_STATUS_TIME > 10000 { // Cada 10 segundos
                let mut status_buf = [0u8; 200];
                let status_len = {
                    let mut cursor = ArrayWriter::new(&mut status_buf);
                    write!(
                        cursor,
                        r#"{{"device":"esp32-actuator-01","led1":{},"led2":{},"led3":{},"timestamp":{}}}"#,
                        led_controller.get_state(1),
                        led_controller.get_state(2), 
                        led_controller.get_state(3),
                        current_time
                    ).unwrap();
                    cursor.pos()
                };

                let _ = mqtt.publish(
                    "esp32/status",
                    QoS::AtLeastOnce,
                    false,
                    &status_buf[..status_len],
                );
                
                LAST_STATUS_TIME = current_time;
            }
        }
        
        FreeRtos::delay_ms(50);
    }
}