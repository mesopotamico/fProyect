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

// Estructura para RFID RC522
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

// Estructura para los botones
struct ButtonManager<'a> {
    button1: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    button2: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    button3: PinDriver<'a, esp_idf_svc::hal::gpio::AnyInputPin, esp_idf_svc::hal::gpio::Input>,
    last_states: [bool; 3],
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
        }
    }
    
    fn check_buttons(&mut self) -> Option<u8> {
        let states = [
            self.button1.is_low(),
            self.button2.is_low(),
            self.button3.is_low(),
        ];
        
        for (i, (&current, &last)) in states.iter().zip(self.last_states.iter()).enumerate() {
            if current && !last { // Detecta flanco descendente (botón presionado)
                self.last_states[i] = current;
                return Some(i as u8 + 1);
            }
            self.last_states[i] = current;
        }
        
        None
    }
}

// Función para leer temperatura de sensor analógico (LM35)
fn read_temperature_sensor(
    adc: &mut AdcDriver,
    adc_channel: &mut AdcChannelDriver<esp_idf_svc::hal::adc::Adc1, esp_idf_svc::hal::gpio::Gpio32>,
) -> Result<f32, esp_idf_svc::sys::EspError> {
    let reading = block!(adc.read(adc_channel))?;
    // LM35: 10mV por °C, con referencia de 3.3V y resolución de 12 bits (4096)
    let voltage = (reading as f32 * 3.3) / 4095.0;
    let temperature = voltage * 100.0; // LM35 da 10mV por °C
    Ok(temperature)
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

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("🚀 ESP32 Device #1 - Sensor & RFID & Buttons");
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

    // Configurar MQTT
    let mqtt_conf = MqttClientConfiguration {
        keep_alive_interval: Some(core::time::Duration::from_secs(30)),
        ..Default::default()
    };

    let (mut mqtt, mut conn) = EspMqttClient::new(
        "mqtt://broker.hivemq.com:1883",
        &mqtt_conf,
    ).unwrap();

    // Maneja la conexión MQTT en thread separado
    std::thread::spawn(move || {
        while conn.next().is_ok() {}
    });

    FreeRtos::delay_ms(1000);
    println!("✅ MQTT conectado");

    // Configurar botones con pull-up interno
    let button1 = PinDriver::input(p.pins.gpio18.downgrade_input()).unwrap();
    let button2 = PinDriver::input(p.pins.gpio19.downgrade_input()).unwrap();  
    let button3 = PinDriver::input(p.pins.gpio21.downgrade_input()).unwrap();
        
    let mut button_manager = ButtonManager::new(button1, button2, button3);
    println!("✅ Botones configurados (GPIO18, 19, 21)");
    
    // Configurar ADC para sensor de temperatura (GPIO32)
    let mut adc1 = AdcDriver::new(p.adc1).unwrap();
    let mut adc1_ch6 = AdcChannelDriver::new(&mut adc1, p.pins.gpio32).unwrap();
    println!("✅ Sensor de temperatura configurado (GPIO32)");

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

    println!("🎯 Sistema listo - presiona botones o acerca tarjeta RFID");

    // Variables de control
    let mut rfid_counter = 0u32;
    let mut last_temp_time = 0u64;

    // Loop principal
    loop {
        let current_time = esp_idf_svc::sys::esp_timer_get_time() / 1000; // ms
        
        // 1. Verificar botones
        if let Some(button_id) = button_manager.check_buttons() {
            println!("🔘 Botón {} presionado!", button_id);
            
            // Crear mensaje JSON para botón
            let mut msg_buf = [0u8; 128];
            let msg_len = {
                let mut cursor = ArrayWriter::new(&mut msg_buf);
                write!(
                    cursor,
                    r#"{{"device":"esp32-sensor-01","button_id":{},"action":"pressed","timestamp":{}}}"#,
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
            
            // Si es botón 1, enviar comando a ESP32 #2 (LED toggle)
            if button_id == 1 {
                let mut cmd_buf = [0u8; 128];
                let cmd_len = {
                    let mut cursor = ArrayWriter::new(&mut cmd_buf);
                    write!(
                        cursor,
                        r#"{{"from":"esp32-sensor-01","to":"esp32-actuator-01","command":"LED_TOGGLE","led_id":1}}"#,
                    ).unwrap();
                    cursor.pos()
                };

                let _ = mqtt.publish(
                    "esp32/commands",
                    QoS::AtLeastOnce,
                    false,
                    &cmd_buf[..cmd_len],
                );
                
                println!("➡️  Comando LED_TOGGLE enviado a ESP32 #2");
            }
            
            // Si es botón 2, activar buzzer en ESP32 #2
            if button_id == 2 {
                let mut cmd_buf = [0u8; 128];
                let cmd_len = {
                    let mut cursor = ArrayWriter::new(&mut cmd_buf);
                    write!(
                        cursor,
                        r#"{{"from":"esp32-sensor-01","to":"esp32-actuator-01","command":"BUZZER","duration":1000}}"#,
                    ).unwrap();
                    cursor.pos()
                };

                let _ = mqtt.publish(
                    "esp32/commands",
                    QoS::AtLeastOnce,
                    false,
                    &cmd_buf[..cmd_len],
                );
                
                println!("🔊 Comando BUZZER enviado a ESP32 #2");
            }
            
            FreeRtos::delay_ms(300); // Debounce
        }
        
        // 2. Leer sensor de temperatura cada 5 segundos
        if current_time - last_temp_time > 5000 {
            if let Ok(temperature) = read_temperature_sensor(&mut adc1, &mut adc1_ch6) {
                println!("🌡️  Temperatura: {:.1}°C", temperature);
                
                // Enviar datos de temperatura
                let mut temp_buf = [0u8; 128];
                let temp_len = {
                    let mut cursor = ArrayWriter::new(&mut temp_buf);
                    write!(
                        cursor,
                        r#"{{"device":"esp32-sensor-01","temp":{:.1},"hum":0.0,"timestamp":{}}}"#,
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
            }
            
            last_temp_time = current_time;
        }
        
        // 3. Verificar tarjeta RFID
        if let Some(_atqa) = rfid.request() {
            if let Some(uid) = rfid.anticoll() {
                rfid_counter += 1;
                
                println!("🏷️  Tarjeta RFID detectada! UID: {:02X}:{:02X}:{:02X}:{:02X} (#{}) ", 
                         uid[0], uid[1], uid[2], uid[3], rfid_counter);
                
                // Crear mensaje JSON para RFID
                let mut rfid_buf = [0u8; 128];
                let rfid_len = {
                    let mut cursor = ArrayWriter::new(&mut rfid_buf);
                    write!(
                        cursor,
                        r#"{{"device":"esp32-sensor-01","uid":"{:02X}{:02X}{:02X}{:02X}","count":{}}}"#,
                        uid[0], uid[1], uid[2], uid[3], rfid_counter
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
        
        FreeRtos::delay_ms(50); // Delay reducido para mejor responsividad
    }
}