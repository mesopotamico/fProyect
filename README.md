#  Sistema IoT ESP32 (In progres)

## **Resumen del Proyecto**

Sistema IoT avanzado con **dos ESP32** que se comunican de forma cruzada, servidor **Rust** con **PostgreSQL**, panel **Node-RED**, y bot de **Telegram**. Todo con protocolos de seguridad **TLS** opcionales.

### ** Funcionalidades Principales**

 **Comunicación cruzada ESP32-a-ESP32**  
 **Sensor de temperatura analógico (LM35)**  
 **RFID RC522 funcional**  
 **Control de LEDs remotos**  
 **Buzzer con notificaciones**  
 **Base de datos PostgreSQL con logs**  
 **API REST completa**  
 **Panel Node-RED en tiempo real**  
 **Bot de Telegram para control remoto**  
 **Seguridad TLS/HTTPS opcional**  

---

##  **Arquitectura del Sistema**

```
┌─────────────────────────────────────────────────────────────────┐
│                    SERVIDOR RUST (Puerto 8123)                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │    Axum      │  │ PostgreSQL   │  │  Telegram    │         │
│  │   Web API    │  │   Database   │  │     Bot      │         │
│  │  + MQTT      │  │   + Logs     │  │ + Commands   │         │
│  └──────────────┘  └──────────────┘  └──────────────┘         │
└─────────────────────────────────────────────────────────────────┘
                                │
                    ┌───────────────────────┐
                    │   MQTT Broker TLS     │
                    │  broker.hivemq.com    │
                    └───────────────────────┘
                                │
        ┌───────────────────────────────────────────┐
        │              Node-RED Panel               │
        │        http://localhost:1880/ui           │
        └───────────────────────────────────────────┘
                                │
        ┌───────────────────────────────────────────┐
        │                                           │
┌───────▼────────┐                        ┌────────▼────────┐
│    ESP32 #1    │◄──────── WiFi ────────►│    ESP32 #2    │
│   (SENSOR)     │    Comunicación        │   (ACTUATOR)   │
├────────────────┤      Cruzada           ├─────────────────┤
│   LM35 Temp   │                         │  3x LEDs      │
│   RFID RC522  │                         │  Buzzer PWM   │
│  3x Botones   │                         │  2x Botones   │
│  WiFi+MQTT    │                         │  WiFi+MQTT    │
└────────────────┘                        └─────────────────┘
```

<img width="668" height="453" alt="Bildschirmfoto 2025-11-27 um 23 37 33" src="https://github.com/user-attachments/assets/7a4128fb-e14d-4bb4-922b-ef00f915faf5" />



---

##  **Estructura del Proyecto**

```
esp32-iot-system/
├── esp32-simulator/         # Servidor Rust Principal
│   ├── src/
│   │   ├── main.rs            # Servidor Axum + MQTT
│   │   ├── database.rs        # PostgreSQL integration
│   │   └── telegram_bot.rs    # Bot de Telegram
│   ├── Cargo.toml
│   ├── .env                   # Configuración (crear desde .env.example)
│   └── database_schema.sql    # Esquema PostgreSQL
├──  esp32-device-1/         # ESP32 Sensor (Temp + RFID + Botones)
│   ├── src/main.rs
│   ├── Cargo.toml
│   └── sdkconfig.defaults
├──  esp32-device-2/         # ESP32 Actuator (LEDs + Buzzer + Botones)  
│   ├── src/main.rs
│   ├── Cargo.toml
│   └── sdkconfig.defaults
├──  node-red-flows/         # Dashboard Node-RED
│   └── esp32-dashboard.json
├──  security/               # Certificados TLS
│   └── generate_certificates.sh
├──  INSTALLATION_GUIDE.md   # Guía detallada de instalación
└──  README.md              # Este archivo
```

---

##  **Inicio Rápido**

### **1. Clonar y Configurar**
```bash
cd /Users/mesopotamico/Desktop/self-learning/rust/IOT-proyect

# Configurar PostgreSQL
createdb esp32_iot
psql -d esp32_iot -f esp32-simulator/database_schema.sql

# Configurar servidor Rust
cd esp32-simulator
cp .env.example .env
# Editar .env con tus configuraciones
nano .env
```

### **2. Ejecutar Servidor Rust**
```bash
cd esp32-simulator
cargo run --release
# Servidor disponible en: http://localhost:8123
```

### **3. Flashear ESP32s**
```bash
# ESP32 Device #1 (Sensor)
cd esp32-device-1  
cargo build --release
espflash flash target/xtensa-esp32-espidf/release/esp32-device-1

# ESP32 Device #2 (Actuator)
cd esp32-device-2
cargo build --release  
espflash flash target/xtensa-esp32-espidf/release/esp32-device-2
```

### **4. Node-RED Dashboard**
```bash
npm install -g node-red node-red-dashboard
node-red
# Ir a http://localhost:1880
# Importar flows desde node-red-flows/esp32-dashboard.json
# Dashboard en: http://localhost:1880/ui
```

---

##  **Conexiones Hardware**

### **ESP32 Device #1 (Sensor):**
```
 Sensor LM35:
   VCC → 3.3V | OUT → GPIO32 | GND → GND

 RFID RC522:  
   VCC → 3.3V | RST → GPIO27 | GND → GND
   SDA → GPIO15 | SCK → GPIO14 | MOSI → GPIO13 | MISO → GPIO12

 Botones (pull-up interno):
   Botón 1 → GPIO18 | Botón 2 → GPIO19 | Botón 3 → GPIO21
```

### **ESP32 Device #2 (Actuator):**
```
 LEDs (con resistencias 220Ω):
   LED 1 → GPIO25 | LED 2 → GPIO26 | LED 3 → GPIO27

 Buzzer PWM:
   + → GPIO21 | - → GND

 Botones (pull-up interno):
   Botón 1 → GPIO18 | Botón 2 → GPIO19
```

---


##  **Endpoints API REST**

Base URL: `http://localhost:8123`

### **Datos de Sensores:**
```http
GET /status                              # Estado general del sistema
GET /data                               # Datos actuales de sensores  
GET /api/sensor_logs/esp32-sensor-01    # Historial de temperatura
GET /api/temperature/latest/esp32-sensor-01  # Última temperatura
```

### **Control de Dispositivos:**
```http
POST /api/device_command                # Enviar comando a ESP32
GET /api/commands/esp32-actuator-01     # Comandos pendientes
GET /rfid                              # Último evento RFID
```

### **Ejemplo - Enviar Comando LED:**
```bash
curl -X POST http://localhost:8123/api/device_command \
  -H "Content-Type: application/json" \
  -d '{
    "from_device": "api-client",
    "to_device": "esp32-actuator-01", 
    "command_type": "LED_ON",
    "command_data": {"led_id": 1}
  }'
```

---

## ** Bot de Telegram**

| Comando | Descripción |
|---------|-------------|
| `/help` |  Lista todos los comandos disponibles |
| `/temperature` | Obtener temperatura actual |
| `/status` |  Estado completo del sistema |
| `/ledon` |  Encender LED en ESP32 #2 |
| `/ledoff` |  Apagar LED en ESP32 #2 |
| `/buzzer` |  Activar buzzer en ESP32 #2 |
| `/rfid` |  Último escaneo de tarjeta RFID |


---

##  **Seguridad TLS (Opcional)**

Para activar encriptación TLS en todo el sistema:

### **Generar Certificados:**
```bash
cd security
chmod +x generate_certificates.sh
./generate_certificates.sh
./copy_certs_to_esp32.sh
```

### **Configurar MQTT Broker Seguro:**
```bash
# Instalar Mosquitto
sudo apt install mosquitto mosquitto-clients

# Ejecutar con TLS
cd security/certs
mosquitto -c mosquitto.conf
```

### **Actualizar ESP32s para TLS:**
- Incluir certificados en `main/certs/`
- Actualizar código para usar `mqtts://`
- Configurar cliente TLS en ESP-IDF

---


-

---

##  **Documentación Completa**

-  **[INSTALLATION_GUIDE.md](INSTALLATION_GUIDE.md)** - Guía paso a paso detallada


---



##  **Autor**

**mesopotamico** - *ESP32 IoT System Developer*  
 n.duque1@utp.edu.co

-
