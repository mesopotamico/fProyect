# 🚀 ESP32 IoT System - Guía de Instalación Completa

## 📋 **Componentes del Sistema**

### **Software:**
- **Servidor Rust** (API + PostgreSQL + Telegram Bot)
- **ESP32 Device #1** (Sensor + RFID + Botones)
- **ESP32 Device #2** (LEDs + Buzzer + Botones)
- **Panel Node-RED** (Dashboard web)

### **Hardware Requerido:**

#### **ESP32 Device #1 (Sensor):**
- ESP32 DevKit
- Sensor de temperatura LM35 (GPIO32)
- RFID RC522 (SPI: GPIO12/13/14/15, RST: GPIO27)
- 3 Botones (GPIO18, 19, 21) + resistencias pull-up
- Protoboard y cables

#### **ESP32 Device #2 (Actuator):**
- ESP32 DevKit
- 3 LEDs (GPIO25, 26, 27) + resistencias 220Ω
- Buzzer pasivo (GPIO21)
- 2 Botones (GPIO18, 19) + resistencias pull-up
- Protoboard y cables

---

## 🗄️ **1. Configuración PostgreSQL**

### **Instalar PostgreSQL:**
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install postgresql postgresql-contrib

# macOS (con Homebrew)
brew install postgresql
brew services start postgresql

# Windows: Descargar desde https://www.postgresql.org/download/
```

### **Crear Base de Datos:**
```bash
# Conectar como usuario postgres
sudo -u postgres psql

# Crear base de datos y usuario
CREATE DATABASE esp32_iot;
CREATE USER esp32_user WITH ENCRYPTED PASSWORD 'esp32_password';
GRANT ALL PRIVILEGES ON DATABASE esp32_iot TO esp32_user;
\q

# Aplicar esquema
psql -U esp32_user -d esp32_iot -f esp32-simulator/database_schema.sql
```

---

## 🦀 **2. Configuración Servidor Rust**

### **Instalar Rust:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### **Configurar Servidor:**
```bash
cd esp32-simulator

# Editar archivo .env con tus configuraciones
cp .env.example .env
nano .env
```

### **Archivo .env:**
```env
# Base de datos PostgreSQL
DATABASE_URL=postgresql://esp32_user:esp32_password@localhost:5432/esp32_iot

# Bot de Telegram (crear en @BotFather)
TELEGRAM_BOT_TOKEN=tu_token_del_bot
TELEGRAM_CHAT_ID=tu_chat_id

# MQTT Configuration
MQTT_BROKER=broker.hivemq.com
MQTT_PORT=1883

# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8123
```

### **Ejecutar Servidor:**
```bash
cargo run --release
```

**El servidor estará disponible en:** `http://localhost:8123`

**Endpoints API disponibles:**
- `GET /status` - Estado del sistema
- `GET /api/sensor_logs/:device_id` - Logs de sensores
- `POST /api/device_command` - Enviar comandos
- `GET /api/commands/:device_id` - Comandos pendientes

---

## 🔧 **3. Configuración ESP32s**

### **Instalar ESP-IDF para Rust:**
```bash
# Instalar espup
cargo install espup
espup install

# Configurar variables de entorno
source ~/export-esp.sh  # Linux/macOS
```

### **ESP32 Device #1 (Sensor):**
```bash
cd esp32-device-1

# Compilar y flashear
cargo build --release
espflash flash target/xtensa-esp32-espidf/release/esp32-device-1

# Monitorear logs
espflash monitor
```

**Conexiones ESP32 #1:**
```
Sensor LM35:
- VCC → 3.3V
- OUT → GPIO32
- GND → GND

RFID RC522:
- VCC → 3.3V  
- RST → GPIO27
- GND → GND
- SDA → GPIO15
- SCK → GPIO14
- MOSI → GPIO13
- MISO → GPIO12

Botones:
- Botón 1 → GPIO18 (pull-up interno)
- Botón 2 → GPIO19 (pull-up interno)  
- Botón 3 → GPIO21 (pull-up interno)
```

### **ESP32 Device #2 (Actuator):**
```bash
cd esp32-device-2

# Compilar y flashear
cargo build --release
espflash flash target/xtensa-esp32-espidf/release/esp32-device-2

# Monitorear logs  
espflash monitor
```

**Conexiones ESP32 #2:**
```
LEDs (con resistencias 220Ω):
- LED 1 → GPIO25 → GND
- LED 2 → GPIO26 → GND
- LED 3 → GPIO27 → GND

Buzzer:
- + → GPIO21
- - → GND

Botones:
- Botón 1 → GPIO18 (pull-up interno)
- Botón 2 → GPIO19 (pull-up interno)
```

---

## 🎨 **4. Configuración Node-RED**

### **Instalar Node-RED:**
```bash
# Instalar Node.js (versión LTS)
# Desde https://nodejs.org/

# Instalar Node-RED globalmente
npm install -g node-red

# Instalar módulos adicionales
npm install -g node-red-dashboard node-red-contrib-ui-toast
```

### **Ejecutar Node-RED:**
```bash
node-red
```

**Acceder a:** `http://localhost:1880`

### **Importar Dashboard:**
1. Ir a **Menú → Import**
2. Pegar contenido de `node-red-flows/esp32-dashboard.json`
3. Hacer clic en **Deploy**
4. Acceder al dashboard en: `http://localhost:1880/ui`

---

## 🤖 **5. Configuración Bot de Telegram**

### **Crear Bot:**
1. Abrir Telegram y buscar `@BotFather`
2. Enviar `/newbot`
3. Seguir instrucciones y obtener token
4. Agregar token al archivo `.env`

### **Obtener Chat ID:**
1. Enviar mensaje a tu bot
2. Ir a: `https://api.telegram.org/botTU_TOKEN/getUpdates`
3. Copiar el `chat.id` al archivo `.env`

### **Comandos Disponibles:**
- `/help` - Mostrar ayuda
- `/temperature` - Temperatura actual
- `/status` - Estado del sistema
- `/ledon` - Encender LED
- `/ledoff` - Apagar LED  
- `/buzzer` - Activar buzzer
- `/rfid` - Último escaneo RFID
- `/logs` - Registros recientes

---

## 🔗 **6. Funcionamiento del Sistema**

### **Comunicación Cruzada:**
- **ESP32 #1 Botón 1** → Enciende LED en ESP32 #2
- **ESP32 #1 Botón 2** → Activa buzzer en ESP32 #2  
- **ESP32 #2 Botón 1** → Toggle LED local
- **ESP32 #2 Botón 2** → Buzzer + envía ACK a ESP32 #1

### **Datos del Sistema:**
- **Temperatura** → Leída cada 5 segundos → Guardada en PostgreSQL
- **RFID** → Evento enviado por MQTT → Guardado en PostgreSQL
- **Botones** → Eventos enviados por MQTT → Comandos cruzados
- **APIs** → Acceso a datos históricos vía REST

### **Interfaces:**
- **Node-RED Dashboard:** Control visual en tiempo real
- **Telegram Bot:** Control remoto por mensaje
- **API REST:** Integración con otros sistemas

---

## 🔒 **7. Configuración de Seguridad (Opcional)**

Para habilitar TLS/SSL:

1. **Generar certificados:**
```bash
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365
```

2. **Configurar MQTT con TLS:**
- Actualizar broker a `mqtts://`
- Configurar certificados en ESP32s

3. **Configurar HTTPS en servidor Rust:**
- Agregar `rustls` al Cargo.toml
- Configurar certificados

---

## 🚨 **Troubleshooting**

### **Problemas Comunes:**

**1. ESP32 no se conecta a WiFi:**
- Verificar credenciales en `sdkconfig.defaults`
- Resetear ESP32 manualmente

**2. PostgreSQL connection failed:**
- Verificar que PostgreSQL esté ejecutándose
- Revisar credenciales en `.env`

**3. MQTT desconectado:**
- Verificar conectividad a internet
- Probar con broker local (mosquitto)

**4. Node-RED no recibe datos:**
- Verificar configuración del broker MQTT
- Revisar tópicos en los flows

### **Logs Útiles:**
```bash
# Servidor Rust
RUST_LOG=debug cargo run

# ESP32
espflash monitor

# Node-RED
# Activar debug en el editor de flows
```

---

## 🎯 **Verificación del Sistema**

### **Checklist de Funcionamiento:**

- [ ] Servidor Rust ejecutándose en puerto 8123
- [ ] PostgreSQL con base de datos creada
- [ ] ESP32 #1 enviando datos de temperatura
- [ ] ESP32 #1 detectando tarjetas RFID  
- [ ] ESP32 #2 recibiendo comandos MQTT
- [ ] LEDs controlables desde Node-RED
- [ ] Buzzer funcional
- [ ] Bot de Telegram respondiendo
- [ ] Dashboard Node-RED mostrando datos en tiempo real
- [ ] Comunicación cruzada entre ESP32s funcionando

### **URLs de Acceso:**
- **API Rust:** http://localhost:8123
- **Node-RED Editor:** http://localhost:1880  
- **Dashboard:** http://localhost:1880/ui
- **PostgreSQL:** puerto 5432
- **MQTT:** broker.hivemq.com:1883

---

## 📚 **Recursos Adicionales**

- **Documentación ESP-IDF:** https://docs.espressif.com/
- **Node-RED Documentation:** https://nodered.org/docs/
- **PostgreSQL Docs:** https://www.postgresql.org/docs/
- **MQTT.org:** https://mqtt.org/
- **Telegram Bot API:** https://core.telegram.org/bots/api

---

¡Sistema IoT ESP32 completo configurado y funcionando! 🎉