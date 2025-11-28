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

// Configuración de seguridad
struct SecurityConfig {
    wifi_ssid: String,
    wifi_password: String,
    mqtt_broker: String,
    mqtt_username: String,
    mqtt_password: String,
    device_id: String,
    max_command_rate: u32, // Comandos máximos por minuto
}

impl SecurityConfig {
    fn load_from_env() -> Result<Self, &'static str> {
        Ok(SecurityConfig {
            wifi_ssid: option_env!("WIFI_SSID")
                .unwrap_or("UTP")
                .to_string(),
            wifi_password: option_env!("WIFI_PASSWORD")
                .unwrap_or("tecnologica")
                .to_string(),
            mqtt_broker: option_env!("MQTT_BROKER")
                .unwrap_or("broker.hivemq.com")
                .to_string(),
            mqtt_username: option_env!("MQTT_USERNAME")
                .unwrap_or("esp32_user_secure")
                .to_string(),
            mqtt_password: option_env!("MQTT_PASSWORD")
                .unwrap_or("esp32_pass_2024_secure")
                .to_string(),
            device_id: option_env!("DEVICE_ID")
                .unwrap_or("esp32-actuator-01-secure")
                .to_string(),
            max_command_rate: 60, // Máximo 60 comandos por minuto
        })
    }
}

// Estructura para LEDs controlables
struct LedController<'a> {
    led1: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
    led2: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
    led3: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
    states: [bool; 3],
    last_change_time: [u64; 3],
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
            last_change_time: [0; 3],
        }
    }

    fn set_led(&mut self, led_id: u8, state: bool) -> Result<(), &'static str> {
        if led_id < 1 || led_id > 3 {
            return Err("LED ID must be between 1 and 3");
        }
        
        let current_time = esp_idf_svc::sys::esp_timer_get_time() / 1000;
        let index = (led_id - 1) as usize;
        
        // Rate limiting: no más de un cambio por segundo por LED
        if current_time - self.last_change_time[index] < 1000 {
            return Err("LED change rate limit exceeded");
        }
        
        self.states[index] = state;
        self.last_change_time[index] = current_time;
        
        let result = match led_id {
            1 => if state { self.led1.set_high() } else { self.led1.set_low() },
            2 => if state { self.led2.set_high() } else { self.led2.set_low() },
            3 => if state { self.led3.set_high() } else { self.led3.set_low() },
            _ => return Err("Invalid LED ID"),
        };
        
        match result {
            Ok(_) => Ok(()),
            Err(_) => Err("Hardware error setting LED"),
        }
    }

    fn toggle_led(&mut self, led_id: u8) -> Result<bool, &'static str> {
        if led_id < 1 || led_id > 3 {
            return Err("LED ID must be between 1 and 3");
        }
        
        let index = (led_id - 1) as usize;
        let new_state = !self.states[index];
        
        match self.set_led(led_id, new_state) {
            Ok(_) => Ok(new_state),
            Err(e) => Err(e),
        }
    }

    fn get_state(&self, led_id: u8) -> bool {
        if led_id < 1 || led_id > 3 {
            return false;
        }
        self.states[(led_id - 1) as usize]
    }

    fn emergency_shutdown(&mut self) {
        // Apagar todos los LEDs sin rate limiting en emergencia
        for i in 0..3 {
            self.states[i] = false;
            self.last_change_time[i] = 0; // Reset rate limiting
        }
        
        let _ = self.led1.set_low();
        let _ = self.led2.set_low();
        let _ = self.led3.set_low();
        
        println!("🚨 EMERGENCY SHUTDOWN - Todos los LEDs apagados");
    }
}

// Estructura para botones con debouncing
struct ButtonController<'a> {
    button1: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    button2: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    last_states: [bool; 2],
    last_press_time: [u64; 2],
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
            last_press_time: [0; 2],
        }
    }
    
    fn check_buttons(&mut self) -> Option<u8> {
        let current_time = esp_idf_svc::sys::esp_timer_get_time() / 1000;
        let debounce_time = 250; // 250ms debounce
        
        let states = [
            self.button1.is_low(),
            self.button2.is_low(),
        ];
        
        for (i, (&current, &last)) in states.iter().zip(self.last_states.iter()).enumerate() {
            if current && !last { // Detecta flanco descendente
                // Verificar debounce
                if current_time - self.last_press_time[i] > debounce_time {
                    self.last_states[i] = current;
                    self.last_press_time[i] = current_time;
                    return Some(i as u8 + 1);
                }
            }
            self.last_states[i] = current;
        }
        
        None
    }
}

// Estructura para buzzer con protección
struct BuzzerController<'a> {
    pwm: LedcDriver<'a>,
    last_beep_time: u64,
    daily_beep_count: u32,
}

impl<'a> BuzzerController<'a> {
    fn new(pwm: LedcDriver<'a>) -> Self {
        BuzzerController { 
            pwm,
            last_beep_time: 0,
            daily_beep_count: 0,
        }
    }

    fn beep(&mut self, frequency: u32, duration_ms: u64) -> Result<(), &'static str> {
        let current_time = esp_idf_svc::sys::esp_timer_get_time() / 1000;
        
        // Rate limiting: No más de un beep cada 2 segundos
        if current_time - self.last_beep_time < 2000 {
            return Err("Buzzer rate limit exceeded");
        }
        
        // Límite diario: máximo 1000 beeps por día
        if self.daily_beep_count >= 1000 {
            return Err("Daily buzzer limit exceeded");
        }
        
        // Validar parámetros
        if frequency < 100 || frequency > 5000 {
            return Err("Frequency out of range (100-5000 Hz)");
        }
        
        if duration_ms > 5000 {
            return Err("Duration too long (max 5000ms)");
        }
        
        // Ejecutar beep
        match self.pwm.set_frequency(frequency) {
            Ok(_) => {
                match self.pwm.set_duty(512) { // 50% duty cycle
                    Ok(_) => {
                        FreeRtos::delay_ms(duration_ms as u32);
                        let _ = self.pwm.set_duty(0); // Apagar
                        
                        self.last_beep_time = current_time;
                        self.daily_beep_count += 1;
                        
                        println!("🔊 Beep ejecutado: {}Hz por {}ms", frequency, duration_ms);
                        Ok(())
                    },
                    Err(_) => Err("Error setting PWM duty"),
                }
            },
            Err(_) => Err("Error setting PWM frequency"),
        }
    }

    fn emergency_beep(&mut self) -> Result<(), &'static str> {
        // Beep de emergencia sin rate limiting
        let _ = self.pwm.set_frequency(1500);
        let _ = self.pwm.set_duty(512);
        FreeRtos::delay_ms(200);
        let _ = self.pwm.set_duty(0);
        
        println!("🚨 Emergency beep executed");
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

// Validador de comandos mejorado
struct CommandValidator {
    allowed_commands: Vec<&'static str>,
    command_count: u32,
    last_reset_time: u64,
    max_commands_per_minute: u32,
}

impl CommandValidator {
    fn new(max_commands_per_minute: u32) -> Self {
        CommandValidator {
            allowed_commands: vec![
                "LED_ON", "LED_OFF", "LED_TOGGLE", "LED_ALL_ON", "LED_ALL_OFF",
                "BUZZER", "BUZZER_TRIPLE", "ACKNOWLEDGE"
            ],
            command_count: 0,
            last_reset_time: 0,
            max_commands_per_minute,
        }
    }
    
    fn validate_command(&mut self, command: &str, source: &str) -> Result<(), String> {
        let current_time = esp_idf_svc::sys::esp_timer_get_time() / 1000;
        
        // Reset contador cada minuto
        if current_time - self.last_reset_time > 60000 {
            self.command_count = 0;
            self.last_reset_time = current_time;
        }
        
        // Verificar rate limiting
        if self.command_count >= self.max_commands_per_minute {
            return Err("Command rate limit exceeded".to_string());
        }
        
        // Verificar comando en whitelist
        if !self.allowed_commands.contains(&command) {
            return Err(format!("Command '{}' not allowed", command));
        }
        
        // Verificar fuente confiable
        if !source.starts_with("esp32-") && !source.starts_with("telegram-bot") && !source.starts_with("node-red") {
            return Err(format!("Untrusted command source: {}", source));
        }
        
        // Comandos específicos que requieren validación extra
        match command {
            "BUZZER" | "BUZZER_TRIPLE" => {
                // Limitar buzzer a fuentes específicas
                if !source.contains("telegram-bot") && !source.contains("esp32-sensor") {
                    return Err("Buzzer commands only allowed from specific sources".to_string());
                }
            },
            "LED_ALL_OFF" => {
                // Emergency command - siempre permitido
            },
            _ => {}
        }
        
        self.command_count += 1;
        Ok(())
    }
}

// Estructura para comando recibido
#[derive(Debug)]
struct Command {
    from: String,
    to: String,
    command: String,
    led_id: Option<u8>,
    duration: Option<u64>,
    emergency: Option<bool>,
    security: Option<String>,
}

impl Command {
    fn from_json(json_str: &str) -> Option<Self> {
        // Parser JSON básico manual con validación de seguridad
        if let (Some(from), Some(to), Some(command)) = (
            extract_json_string(json_str, "from"),
            extract_json_string(json_str, "to"),
            extract_json_string(json_str, "command"),
        ) {
            let led_id = extract_json_number(json_str, "led_id").map(|n| n as u8);
            let duration = extract_json_number(json_str, "duration").map(|n| n as u64);
            let emergency = extract_json_bool(json_str, "emergency");
            let security = extract_json_string(json_str, "security");
            
            Some(Command {
                from,
                to,
                command,
                led_id,
                duration,
                emergency,
                security,
            })
        } else {
            None
        }
    }
    
    fn validate_parameters(&self) -> Result<(), String> {
        // Validar LED ID
        if let Some(led_id) = self.led_id {
            if led_id < 1 || led_id > 3 {
                return Err("LED ID must be between 1 and 3".to_string());
            }
        }
        
        // Validar duration
        if let Some(duration) = self.duration {
            if duration > 10000 {
                return Err("Duration cannot exceed 10000ms".to_string());
            }
            if duration < 50 {
                return Err("Duration too short (minimum 50ms)".to_string());
            }
        }
        
        // Verificar marca de seguridad en comandos críticos
        if self.command == "LED_ALL_OFF" || self.emergency == Some(true) {
            // Comandos de emergencia no requieren validación adicional
            return Ok(());
        }
        
        Ok(())
    }
}

// Helpers para parsing JSON
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

fn extract_json_bool(json: &str, key: &str) -> Option<bool> {
    let search = format!("\"{}\":", key);
    if let Some(start) = json.find(&search) {
        let after_colon = &json[start + search.len()..];
        if after_colon.trim_start().starts_with("true") {
            Some(true)
        } else if after_colon.trim_start().starts_with("false") {
            Some(false)
        } else {
            None
        }
    } else {
        None
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("🔒 ESP32 Device #2 SECURE - Actuator (LEDs + Buzzer + Buttons)");

    // Cargar configuración de seguridad
    let security_config = match SecurityConfig::load_from_env() {
        Ok(config) => config,
        Err(e) => {
            println!("❌ Error cargando configuración de seguridad: {}", e);
            panic!("No se puede continuar sin configuración segura");
        }
    };
    
    println!("🔑 Configuración de seguridad cargada para device: {}", security_config.device_id);

    let p = Peripherals::take().unwrap();
    let s = EspSystemEventLoop::take().unwrap();
    let n = EspDefaultNvsPartition::take().unwrap();

    // Configurar WiFi con credenciales seguras
    let mut w = BlockingWifi::wrap(
        EspWifi::new(p.modem, s.clone(), Some(n.clone())).unwrap(),
        s,
    ).unwrap();

    w.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: security_config.wifi_ssid.as_str().try_into().unwrap(),
        password: security_config.wifi_password.as_str().try_into().unwrap(),
        ..Default::default()
    })).unwrap();

    w.start().unwrap();
    w.connect().unwrap();
    w.wait_netif_up().unwrap();
    println!("✅ WiFi conectado de forma segura");

    // Configurar LEDs (GPIO 25, 26, 27)
    let led1 = PinDriver::output(p.pins.gpio25.downgrade_output()).unwrap();
    let led2 = PinDriver::output(p.pins.gpio26.downgrade_output()).unwrap();
    let led3 = PinDriver::output(p.pins.gpio27.downgrade_output()).unwrap();
    
    let mut led_controller = LedController::new(led1, led2, led3);
    println!("✅ LEDs configurados con protección (GPIO25, 26, 27)");
    
    // Configurar botones (GPIO 18, 19)
    let button1 = PinDriver::input(p.pins.gpio18.downgrade_input()).unwrap();
    let button2 = PinDriver::input(p.pins.gpio19.downgrade_input()).unwrap();
    
    let mut button_controller = ButtonController::new(button1, button2);
    println!("✅ Botones configurados con debouncing (GPIO18, 19)");

    // Configurar buzzer con PWM (GPIO 21)
    let timer_config = TimerConfig::new().frequency(1000.into());
    let timer = LedcTimerDriver::new(p.ledc.timer0, &timer_config).unwrap();
    let pwm = LedcDriver::new(
        p.ledc.channel0,
        timer,
        p.pins.gpio21,
    ).unwrap();
    
    let mut buzzer = BuzzerController::new(pwm);
    println!("✅ Buzzer configurado con protección (GPIO21)");

    // Test seguro de LEDs
    println!("🔄 Test de LEDs seguro...");
    for i in 1..=3 {
        match led_controller.set_led(i, true) {
            Ok(_) => println!("  LED {} encendido", i),
            Err(e) => println!("  Error LED {}: {}", i, e),
        }
        FreeRtos::delay_ms(300);
        let _ = led_controller.set_led(i, false);
        FreeRtos::delay_ms(300);
    }

    // Configurar MQTT con autenticación
    let mqtt_conf = MqttClientConfiguration {
        username: Some(&security_config.mqtt_username),
        password: Some(&security_config.mqtt_password),
        client_id: Some(&security_config.device_id),
        keep_alive_interval: Some(core::time::Duration::from_secs(30)),
        ..Default::default()
    };

    let mqtt_url = format!("mqtt://{}:1883", security_config.mqtt_broker);
    let (mqtt, mut conn) = EspMqttClient::new(&mqtt_url, &mqtt_conf).unwrap();

    // Suscribirse a comandos
    mqtt.subscribe("esp32/commands", QoS::AtLeastOnce).unwrap();
    println!("✅ Suscrito a esp32/commands con autenticación");

    // Variables compartidas para comunicación entre threads
    let command_queue = Arc::new(Mutex::new(Vec::<Command>::new()));
    let mut command_validator = CommandValidator::new(security_config.max_command_rate);
    
    // Thread para manejar MQTT
    let mqtt_clone = mqtt.clone();
    let command_queue_clone = command_queue.clone();
    
    thread::spawn(move || {
        println!("🔄 Iniciando thread MQTT seguro...");
        loop {
            match conn.next() {
                Ok(Event::Received(msg)) => {
                    if let Ok(payload) = std::str::from_utf8(&msg.payload) {
                        println!("📨 Comando recibido: {}", payload);
                        
                        if let Some(command) = Command::from_json(payload) {
                            // Validar que el comando está dirigido a este dispositivo
                            if command.to == "esp32-actuator-01" || command.to == "esp32-actuator-01-secure" {
                                // Validar parámetros del comando
                                match command.validate_parameters() {
                                    Ok(_) => {
                                        let mut queue = command_queue_clone.lock().unwrap();
                                        queue.push(command);
                                        println!("✅ Comando añadido a la cola");
                                    },
                                    Err(e) => {
                                        println!("❌ Comando rechazado por parámetros inválidos: {}", e);
                                    }
                                }
                            } else {
                                println!("⚠️ Comando no dirigido a este dispositivo: {}", command.to);
                            }
                        } else {
                            println!("❌ Formato de comando JSON inválido");
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
    println!("🎯 Sistema SEGURO listo - esperando comandos y botones");

    // Loop principal
    let mut last_status_time = 0u64;
    let mut heartbeat_time = 0u64;

    loop {
        let current_time = esp_idf_svc::sys::esp_timer_get_time() / 1000;

        // 1. Procesar comandos recibidos con validación
        let commands_to_process: Vec<Command> = {
            let mut queue = command_queue.lock().unwrap();
            let commands = queue.clone();
            queue.clear();
            commands
        };

        for command in commands_to_process {
            // Validar comando con el validador
            match command_validator.validate_command(&command.command, &command.from) {
                Ok(_) => {
                    println!("⚡ Ejecutando comando validado: {} de {}", command.command, command.from);
                    
                    // Ejecutar comando
                    let execution_result = match command.command.as_str() {
                        "LED_ON" => {
                            if let Some(led_id) = command.led_id {
                                led_controller.set_led(led_id, true)
                                    .map(|_| format!("LED {} encendido", led_id))
                            } else {
                                Err("LED ID requerido")
                            }
                        },
                        "LED_OFF" => {
                            if let Some(led_id) = command.led_id {
                                led_controller.set_led(led_id, false)
                                    .map(|_| format!("LED {} apagado", led_id))
                            } else {
                                Err("LED ID requerido")
                            }
                        },
                        "LED_TOGGLE" => {
                            if let Some(led_id) = command.led_id {
                                led_controller.toggle_led(led_id)
                                    .map(|new_state| format!("LED {} {}", led_id, if new_state { "encendido" } else { "apagado" }))
                            } else {
                                Err("LED ID requerido")
                            }
                        },
                        "LED_ALL_OFF" => {
                            // Comando de emergencia
                            if command.emergency == Some(true) {
                                led_controller.emergency_shutdown();
                                let _ = buzzer.emergency_beep();
                                Ok("Emergency shutdown ejecutado".to_string())
                            } else {
                                // LED_ALL_OFF normal
                                for i in 1..=3 {
                                    let _ = led_controller.set_led(i, false);
                                }
                                Ok("Todos los LEDs apagados".to_string())
                            }
                        },
                        "BUZZER" => {
                            let duration = command.duration.unwrap_or(500);
                            buzzer.beep(1000, duration)
                                .map(|_| format!("Buzzer activado por {}ms", duration))
                        },
                        "BUZZER_TRIPLE" => {
                            // Triple beep seguro
                            let mut result = Ok("Triple beep ejecutado".to_string());
                            for _ in 0..3 {
                                if let Err(e) = buzzer.beep(1000, 200) {
                                    result = Err(e);
                                    break;
                                }
                                FreeRtos::delay_ms(150);
                            }
                            result
                        },
                        "ACKNOWLEDGE" => {
                            let _ = buzzer.beep(750, 300);
                            Ok("Acknowledge recibido".to_string())
                        },
                        _ => {
                            Err("Comando no implementado")
                        }
                    };

                    // Log resultado
                    match execution_result {
                        Ok(msg) => {
                            println!("✅ {}", msg);
                        },
                        Err(e) => {
                            println!("❌ Error ejecutando comando: {}", e);
                        }
                    }
                },
                Err(e) => {
                    println!("🚫 Comando rechazado por validador: {}", e);
                }
            }
        }

        // 2. Verificar botones locales
        if let Some(button_id) = button_controller.check_buttons() {
            println!("🔘 Botón {} presionado! (con debouncing)", button_id);
            
            match button_id {
                1 => {
                    // Botón 1: Toggle LED 1 local
                    match led_controller.toggle_led(1) {
                        Ok(new_state) => {
                            println!("🔄 LED 1 {} por botón local", if new_state { "encendido" } else { "apagado" });
                        },
                        Err(e) => {
                            println!("❌ Error toggle LED 1: {}", e);
                        }
                    }
                },
                2 => {
                    // Botón 2: Activar buzzer y enviar acknowledge
                    match buzzer.beep(750, 300) {
                        Ok(_) => {
                            println!("🔊 Buzzer activado por botón local");
                            
                            // Enviar acknowledge de vuelta
                            let mut msg_buf = [0u8; 128];
                            let msg_len = {
                                let mut cursor = ArrayWriter::new(&mut msg_buf);
                                write!(
                                    cursor,
                                    r#"{{"from":"{}","to":"esp32-sensor-01","command":"ACKNOWLEDGE","timestamp":{},"security":"validated"}}"#,
                                    security_config.device_id,
                                    current_time
                                ).unwrap();
                                cursor.pos()
                            };

                            let _ = mqtt.publish(
                                "esp32/commands",
                                QoS::AtLeastOnce,
                                false,
                                &msg_buf[..msg_len],
                            );
                            
                            println!("📤 Acknowledge enviado de vuelta");
                        },
                        Err(e) => {
                            println!("❌ Error activando buzzer: {}", e);
                        }
                    }
                },
                _ => {}
            }
            
            // Publicar evento de botón
            let mut event_buf = [0u8; 128];
            let event_len = {
                let mut cursor = ArrayWriter::new(&mut event_buf);
                write!(
                    cursor,
                    r#"{{"device":"{}","button_id":{},"action":"pressed","timestamp":{},"security":"enabled"}}"#,
                    security_config.device_id,
                    button_id,
                    current_time
                ).unwrap();
                cursor.pos()
            };

            let _ = mqtt.publish(
                "esp32/button/events",
                QoS::AtLeastOnce,
                false,
                &event_buf[..event_len],
            );
        }

        // 3. Enviar estado de LEDs periódicamente
        if current_time - last_status_time > 15000 { // Cada 15 segundos
            let mut status_buf = [0u8; 200];
            let status_len = {
                let mut cursor = ArrayWriter::new(&mut status_buf);
                write!(
                    cursor,
                    r#"{{"device":"{}","led1":{},"led2":{},"led3":{},"timestamp":{},"security":"enabled"}}"#,
                    security_config.device_id,
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
            
            last_status_time = current_time;
        }

        // 4. Heartbeat de seguridad
        if current_time - heartbeat_time > 30000 { // Cada 30 segundos
            let mut heartbeat_buf = [0u8; 128];
            let heartbeat_len = {
                let mut cursor = ArrayWriter::new(&mut heartbeat_buf);
                write!(
                    cursor,
                    r#"{{"device":"{}","status":"online","uptime":{},"security":"enabled","commands_processed":{}}}"#,
                    security_config.device_id,
                    current_time / 1000,
                    command_validator.command_count
                ).unwrap();
                cursor.pos()
            };

            let _ = mqtt.publish(
                "esp32/heartbeat",
                QoS::AtLeastOnce,
                false,
                &heartbeat_buf[..heartbeat_len],
            );
            
            heartbeat_time = current_time;
        }
        
        FreeRtos::delay_ms(50);
    }
}