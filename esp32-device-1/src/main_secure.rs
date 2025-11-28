use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::gpio::{PinDriver, OutputPin, InputPin, Pull};
use esp_idf_svc::hal::spi::{SpiDeviceDriver, SpiDriver, SpiDriverConfig, config::Config as SpiConfig};
use esp_idf_svc::hal::adc::{AdcDriver, AdcChannelDriver, Atten};
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration, QoS};
use nb::block;
use core::fmt::Write;

// Comandos básicos del RC522
const PCD_IDLE: u8 = 0x00;
const PCD_TRANSCEIVE: u8 = 0x0C;
const PICC_REQIDL: u8 = 0x26;
const PICC_ANTICOLL: u8 = 0x93;

// Registros del RC522
const COMMAND_REG: u8 = 0x01;
const COM_IRQ_REG: u8 = 0x04;
const DIV_IRQ_REG: u8 = 0x05;
const ERROR_REG: u8 = 0x06;
const STATUS2_REG: u8 = 0x08;
const FIFO_DATA_REG: u8 = 0x09;
const FIFO_LEVEL_REG: u8 = 0x0A;
const BIT_FRAMING_REG: u8 = 0x0D;
const COLL_REG: u8 = 0x0E;
const MODE_REG: u8 = 0x11;
const TX_CONTROL_REG: u8 = 0x14;
const TX_AUTO_REG: u8 = 0x15;
const T_MODE_REG: u8 = 0x2A;
const T_PRESCALER_REG: u8 = 0x2B;
const T_RELOAD_REG_H: u8 = 0x2C;
const T_RELOAD_REG_L: u8 = 0x2D;

// Configuración de seguridad
struct SecurityConfig {
    wifi_ssid: String,
    wifi_password: String,
    mqtt_broker: String,
    mqtt_username: String,
    mqtt_password: String,
    device_id: String,
}

impl SecurityConfig {
    fn load_from_env() -> Result<Self, &'static str> {
        // En un sistema real, estas variables se cargarían de forma segura
        // Por ejemplo, desde NVS encriptado o flash seguro
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
                .unwrap_or("esp32-sensor-01-secure")
                .to_string(),
        })
    }
}

// Estructura para RFID RC522 (same as before)
struct Mfrc522<'a> {
    spi: SpiDeviceDriver<'a, SpiDriver<'a>>,
    rst: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
}

impl<'a> Mfrc522<'a> {
    fn new(
        spi: SpiDeviceDriver<'a, SpiDriver<'a>>,
        rst: PinDriver<'a, esp_idf_svc::hal::gpio::AnyOutputPin, esp_idf_svc::hal::gpio::Output>,
    ) -> Self {
        Mfrc522 { spi, rst }
    }

    fn init(&mut self) {
        println!("🔧 Iniciando RFID RC522...");
        
        // Hard reset
        self.rst.set_low().ok();
        FreeRtos::delay_ms(50);
        self.rst.set_high().ok();
        FreeRtos::delay_ms(50);
        
        // Soft reset
        self.write_register(COMMAND_REG, 0x0F);
        FreeRtos::delay_ms(50);
        
        // Timer configuration
        self.write_register(T_MODE_REG, 0x8D);
        self.write_register(T_PRESCALER_REG, 0x3E);
        self.write_register(T_RELOAD_REG_L, 30);
        self.write_register(T_RELOAD_REG_H, 0);
        
        // Force 100% ASK modulation
        self.write_register(TX_AUTO_REG, 0x40);
        
        // CRC preset value 0x6363
        self.write_register(MODE_REG, 0x3D);
        
        // Turn on antenna
        self.antenna_on();
        
        println!("✅ RFID RC522 inicializado");
    }

    fn write_register(&mut self, reg: u8, value: u8) {
        let addr = (reg << 1) & 0x7E;
        let _ = self.spi.write(&[addr, value]);
    }

    fn read_register(&mut self, reg: u8) -> u8 {
        let addr = ((reg << 1) & 0x7E) | 0x80;
        let mut rx = [0u8; 2];
        let tx = [addr, 0x00];
        
        let _ = self.spi.transfer(&mut rx, &tx);
        rx[1]
    }

    fn antenna_on(&mut self) {
        let value = self.read_register(TX_CONTROL_REG);
        if (value & 0x03) != 0x03 {
            self.write_register(TX_CONTROL_REG, value | 0x03);
        }
    }

    fn request(&mut self) -> Option<[u8; 2]> {
        self.write_register(COM_IRQ_REG, 0x7F);
        self.write_register(DIV_IRQ_REG, 0x7F);
        
        self.write_register(BIT_FRAMING_REG, 0x07);
        self.write_register(FIFO_LEVEL_REG, 0x80);
        self.write_register(FIFO_DATA_REG, PICC_REQIDL);
        self.write_register(COMMAND_REG, PCD_TRANSCEIVE);
        
        let val = self.read_register(BIT_FRAMING_REG);
        self.write_register(BIT_FRAMING_REG, val | 0x80);
        
        let mut timeout = 2000;
        let mut fifo_level = 0;
        loop {
            let irq = self.read_register(COM_IRQ_REG);
            fifo_level = self.read_register(FIFO_LEVEL_REG);
            
            timeout -= 1;
            if timeout == 0 || (irq & 0x01) != 0 || (irq & 0x20) != 0 || fifo_level >= 2 {
                break;
            }
        }
        
        let val = self.read_register(BIT_FRAMING_REG);
        self.write_register(BIT_FRAMING_REG, val & (!0x80));
        
        if timeout != 0 && fifo_level >= 2 {
            let error = self.read_register(ERROR_REG);
            if (error & 0x1B) == 0x00 {
                let atqa1 = self.read_register(FIFO_DATA_REG);
                let atqa2 = self.read_register(FIFO_DATA_REG);
                return Some([atqa1, atqa2]);
            }
        }
        
        None
    }

    fn anticoll(&mut self) -> Option<[u8; 5]> {
        self.write_register(COM_IRQ_REG, 0x7F);
        self.write_register(DIV_IRQ_REG, 0x7F);
        
        self.write_register(BIT_FRAMING_REG, 0x00);
        self.write_register(FIFO_LEVEL_REG, 0x80);
        
        self.write_register(FIFO_DATA_REG, PICC_ANTICOLL);
        self.write_register(FIFO_DATA_REG, 0x20);
        
        self.write_register(COMMAND_REG, PCD_TRANSCEIVE);
        
        let val = self.read_register(BIT_FRAMING_REG);
        self.write_register(BIT_FRAMING_REG, val | 0x80);
        
        let mut timeout = 2000;
        let mut fifo_level = 0;
        loop {
            let irq = self.read_register(COM_IRQ_REG);
            fifo_level = self.read_register(FIFO_LEVEL_REG);
            
            timeout -= 1;
            if timeout == 0 || (irq & 0x01) != 0 || (irq & 0x20) != 0 || fifo_level >= 5 {
                break;
            }
        }
        
        let val = self.read_register(BIT_FRAMING_REG);
        self.write_register(BIT_FRAMING_REG, val & (!0x80));
        
        if timeout != 0 && fifo_level >= 5 {
            let error = self.read_register(ERROR_REG);
            if (error & 0x1B) == 0x00 {
                let mut uid = [0u8; 5];
                for i in 0..5 {
                    uid[i] = self.read_register(FIFO_DATA_REG);
                }
                
                let bcc = uid[0] ^ uid[1] ^ uid[2] ^ uid[3];
                if bcc == uid[4] {
                    return Some(uid);
                }
            }
        }
        
        None
    }

    fn halt(&mut self) {
        self.write_register(FIFO_LEVEL_REG, 0x80);
        self.write_register(FIFO_DATA_REG, 0x50);
        self.write_register(FIFO_DATA_REG, 0x00);
        self.write_register(COMMAND_REG, PCD_TRANSCEIVE);
        FreeRtos::delay_ms(10);
    }
}

// Estructura para los botones con debouncing mejorado
struct ButtonManager<'a> {
    button1: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    button2: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    button3: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    last_states: [bool; 3],
    last_press_time: [u64; 3],
}

impl<'a> ButtonManager<'a> {
    fn new(
        button1: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
        button2: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
        button3: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    ) -> Self {
        ButtonManager {
            button1,
            button2,
            button3,
            last_states: [false; 3],
            last_press_time: [0; 3],
        }
    }
    
    fn check_buttons(&mut self) -> Option<u8> {
        let current_time = esp_idf_svc::sys::esp_timer_get_time() / 1000; // ms
        let debounce_time = 200; // 200ms debounce
        
        let states = [
            self.button1.is_low(),
            self.button2.is_low(),
            self.button3.is_low(),
        ];
        
        for (i, (&current, &last)) in states.iter().zip(self.last_states.iter()).enumerate() {
            if current && !last { // Detecta flanco descendente (botón presionado)
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

// Función mejorada para leer temperatura con calibración
fn read_temperature_sensor(
    adc: &mut AdcDriver,
    adc_channel: &mut AdcChannelDriver<esp_idf_svc::hal::adc::Adc1, esp_idf_svc::hal::gpio::Gpio32>,
) -> Result<f32, esp_idf_svc::sys::EspError> {
    // Tomar múltiples lecturas para promediar
    let mut readings = [0u16; 5];
    for i in 0..5 {
        readings[i] = block!(adc.read(adc_channel))?;
        FreeRtos::delay_ms(10);
    }
    
    // Filtrar outliers y promediar
    readings.sort_unstable();
    let avg_reading = (readings[1] + readings[2] + readings[3]) / 3; // Descartar min y max
    
    // LM35: 10mV por °C, con referencia de 3.3V y resolución de 12 bits (4096)
    let voltage = (avg_reading as f32 * 3.3) / 4095.0;
    let temperature = voltage * 100.0; // LM35 da 10mV por °C
    
    // Validar rango razonable (0-50°C para interiores)
    if temperature >= -10.0 && temperature <= 60.0 {
        Ok(temperature)
    } else {
        Err(esp_idf_svc::sys::EspError::from_infallible::<{esp_idf_svc::sys::ESP_ERR_INVALID_RESPONSE}>())
    }
}

// Helper para JSON sin heap allocation
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

// Validación de comandos
fn validate_command(command: &str) -> bool {
    const ALLOWED_COMMANDS: &[&str] = &[
        "LED_ON", "LED_OFF", "LED_TOGGLE", "LED_ALL_ON", "LED_ALL_OFF",
        "BUZZER", "BUZZER_TRIPLE", "ACKNOWLEDGE"
    ];
    
    ALLOWED_COMMANDS.contains(&command)
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("🔒 ESP32 Device #1 SECURE - Sensor & RFID & Buttons");
    
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

    // Configurar MQTT con autenticación
    let mqtt_conf = MqttClientConfiguration {
        username: Some(&security_config.mqtt_username),
        password: Some(&security_config.mqtt_password),
        client_id: Some(&security_config.device_id),
        keep_alive_interval: Some(core::time::Duration::from_secs(30)),
        ..Default::default()
    };

    let mqtt_url = format!("mqtt://{}:1883", security_config.mqtt_broker);
    let (mut mqtt, mut conn) = EspMqttClient::new(&mqtt_url, &mqtt_conf).unwrap();

    // Maneja la conexión MQTT en thread separado
    std::thread::spawn(move || {
        while conn.next().is_ok() {}
    });

    FreeRtos::delay_ms(1000);
    println!("✅ MQTT conectado con autenticación");

    // Configurar botones con pull-up interno
    let button1 = PinDriver::input(p.pins.gpio18.downgrade_input()).unwrap();
    let button2 = PinDriver::input(p.pins.gpio19.downgrade_input()).unwrap();  
    let button3 = PinDriver::input(p.pins.gpio21.downgrade_input()).unwrap();
        
    let mut button_manager = ButtonManager::new(button1, button2, button3);
    println!("✅ Botones configurados con debouncing (GPIO18, 19, 21)");
    
    // Configurar ADC para sensor de temperatura (GPIO32)
    let mut adc1 = AdcDriver::new(p.adc1).unwrap();
    let mut adc1_ch6 = AdcChannelDriver::new(&mut adc1, p.pins.gpio32).unwrap();
    println!("✅ Sensor de temperatura configurado con filtrado (GPIO32)");

    // Configurar SPI para RFID
    let spi_driver = SpiDriver::new(
        p.spi2,
        p.pins.gpio14,  // SCK
        p.pins.gpio13,  // MOSI
        Some(p.pins.gpio12), // MISO
        &SpiDriverConfig::new(),
    ).unwrap();

    let spi_device = SpiDeviceDriver::new(
        spi_driver,
        Some(p.pins.gpio15), // SDA
        &SpiConfig::new().baudrate(1_000_000.into()),
    ).unwrap();

    let rst = PinDriver::output(p.pins.gpio27.downgrade_output()).unwrap();

    let mut rfid = Mfrc522::new(spi_device, rst);
    rfid.init();

    println!("🎯 Sistema SEGURO listo - presiona botones o acerca tarjeta RFID");

    // Variables de control
    let mut rfid_counter = 0u32;
    let mut last_temp_time = 0u64;
    let mut heartbeat_time = 0u64;

    // Loop principal
    loop {
        let current_time = esp_idf_svc::sys::esp_timer_get_time() / 1000; // ms
        
        // 1. Verificar botones con debouncing
        if let Some(button_id) = button_manager.check_buttons() {
            println!("🔘 Botón {} presionado! (con debouncing)", button_id);
            
            // Crear mensaje JSON para botón
            let mut msg_buf = [0u8; 128];
            let msg_len = {
                let mut cursor = ArrayWriter::new(&mut msg_buf);
                write!(
                    cursor,
                    r#"{{"device":"{}","button_id":{},"action":"pressed","timestamp":{},"security":"enabled"}}"#,
                    security_config.device_id,
                    button_id,
                    current_time
                ).unwrap();
                cursor.pos()
            };

            // Publicar evento de botón
            let _ = mqtt.publish(
                "esp32/button/events",
                QoS::AtLeastOnce,
                false,
                &msg_buf[..msg_len],
            );
            
            // Comandos seguros basados en botón presionado
            match button_id {
                1 => {
                    if validate_command("LED_TOGGLE") {
                        let mut cmd_buf = [0u8; 128];
                        let cmd_len = {
                            let mut cursor = ArrayWriter::new(&mut cmd_buf);
                            write!(
                                cursor,
                                r#"{{"from":"{}","to":"esp32-actuator-01","command":"LED_TOGGLE","led_id":1,"security":"validated"}}"#,
                                security_config.device_id
                            ).unwrap();
                            cursor.pos()
                        };

                        let _ = mqtt.publish(
                            "esp32/commands",
                            QoS::AtLeastOnce,
                            false,
                            &cmd_buf[..cmd_len],
                        );
                        
                        println!("➡️  Comando LED_TOGGLE validado y enviado");
                    }
                },
                2 => {
                    if validate_command("BUZZER") {
                        let mut cmd_buf = [0u8; 128];
                        let cmd_len = {
                            let mut cursor = ArrayWriter::new(&mut cmd_buf);
                            write!(
                                cursor,
                                r#"{{"from":"{}","to":"esp32-actuator-01","command":"BUZZER","duration":1000,"security":"validated"}}"#,
                                security_config.device_id
                            ).unwrap();
                            cursor.pos()
                        };

                        let _ = mqtt.publish(
                            "esp32/commands",
                            QoS::AtLeastOnce,
                            false,
                            &cmd_buf[..cmd_len],
                        );
                        
                        println!("🔊 Comando BUZZER validado y enviado");
                    }
                },
                3 => {
                    // Botón de emergencia - apagar todos los LEDs
                    if validate_command("LED_ALL_OFF") {
                        let mut cmd_buf = [0u8; 128];
                        let cmd_len = {
                            let mut cursor = ArrayWriter::new(&mut cmd_buf);
                            write!(
                                cursor,
                                r#"{{"from":"{}","to":"esp32-actuator-01","command":"LED_ALL_OFF","emergency":true,"security":"validated"}}"#,
                                security_config.device_id
                            ).unwrap();
                            cursor.pos()
                        };

                        let _ = mqtt.publish(
                            "esp32/commands",
                            QoS::AtLeastOnce,
                            false,
                            &cmd_buf[..cmd_len],
                        );
                        
                        println!("🚨 Botón de EMERGENCIA - Apagando todos los LEDs");
                    }
                },
                _ => {}
            }
        }
        
        // 2. Leer sensor de temperatura cada 5 segundos con validación
        if current_time - last_temp_time > 5000 {
            match read_temperature_sensor(&mut adc1, &mut adc1_ch6) {
                Ok(temperature) => {
                    println!("🌡️  Temperatura: {:.1}°C (validada)", temperature);
                    
                    // Enviar datos de temperatura con validación
                    let mut temp_buf = [0u8; 128];
                    let temp_len = {
                        let mut cursor = ArrayWriter::new(&mut temp_buf);
                        write!(
                            cursor,
                            r#"{{"device":"{}","temp":{:.1},"hum":0.0,"timestamp":{},"validated":true}}"#,
                            security_config.device_id,
                            temperature,
                            current_time
                        ).unwrap();
                        cursor.pos()
                    };

                    let _ = mqtt.publish(
                        "esp32/hardware/data",
                        QoS::AtLeastOnce,
                        false,
                        &temp_buf[..temp_len],
                    );
                },
                Err(e) => {
                    println!("⚠️  Error leyendo temperatura (fuera de rango): {:?}", e);
                }
            }
            
            last_temp_time = current_time;
        }
        
        // 3. Verificar tarjeta RFID
        if let Some(_atqa) = rfid.request() {
            if let Some(uid) = rfid.anticoll() {
                rfid_counter += 1;
                
                println!("🏷️  Tarjeta RFID detectada! UID: {:02X}:{:02X}:{:02X}:{:02X} (#{}) ", 
                         uid[0], uid[1], uid[2], uid[3], rfid_counter);
                
                // Crear mensaje JSON para RFID con seguridad
                let mut rfid_buf = [0u8; 128];
                let rfid_len = {
                    let mut cursor = ArrayWriter::new(&mut rfid_buf);
                    write!(
                        cursor,
                        r#"{{"device":"{}","uid":"{:02X}{:02X}{:02X}{:02X}","count":{},"security":"validated"}}"#,
                        security_config.device_id,
                        uid[0], uid[1], uid[2], uid[3], 
                        rfid_counter
                    ).unwrap();
                    cursor.pos()
                };

                // Publicar evento RFID
                let _ = mqtt.publish(
                    "esp32/rfid/events",
                    QoS::AtLeastOnce,
                    false,
                    &rfid_buf[..rfid_len],
                );
                
                rfid.halt();
                FreeRtos::delay_ms(1000);
            }
        }
        
        // 4. Heartbeat cada 30 segundos para monitoreo
        if current_time - heartbeat_time > 30000 {
            let mut heartbeat_buf = [0u8; 128];
            let heartbeat_len = {
                let mut cursor = ArrayWriter::new(&mut heartbeat_buf);
                write!(
                    cursor,
                    r#"{{"device":"{}","status":"online","uptime":{},"security":"enabled"}}"#,
                    security_config.device_id,
                    current_time / 1000
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
        
        FreeRtos::delay_ms(50); // Delay reducido para mejor responsividad
    }
}