# 🔐 Análisis de Seguridad - Sistema IoT ESP32

## 📊 **Actividad 1 - Identificación de Activos**

### **🖥️ Hardware**

| Activo | Confidencialidad | Integridad | Disponibilidad | Importancia |
|--------|------------------|------------|----------------|-------------|
| ESP32 Device #1 (Sensor) | MEDIA | ALTA | ALTA | CRÍTICO |
| ESP32 Device #2 (Actuator) | BAJA | ALTA | MEDIA | ALTO |
| Servidor Rust (localhost) | ALTA | ALTA | ALTA | CRÍTICO |
| Sensor LM35 | BAJA | MEDIA | MEDIA | MEDIO |
| RFID RC522 | MEDIA | MEDIA | BAJA | MEDIO |
| LEDs y Buzzer | BAJA | BAJA | BAJA | BAJO |

### **💻 Software**

| Activo | Confidencialidad | Integridad | Disponibilidad | Importancia |
|--------|------------------|------------|----------------|-------------|
| Firmware ESP32 | ALTA | ALTA | ALTA | CRÍTICO |
| Servidor Rust (Axum) | ALTA | ALTA | ALTA | CRÍTICO |
| PostgreSQL Database | ALTA | ALTA | ALTA | CRÍTICO |
| Node-RED Flows | MEDIA | ALTA | MEDIA | ALTO |
| Bot Telegram | ALTA | ALTA | MEDIA | ALTO |
| Sistema Operativo Host | ALTA | ALTA | ALTA | CRÍTICO |

### **🌐 Comunicación**

| Activo | Confidencialidad | Integridad | Disponibilidad | Importancia |
|--------|------------------|------------|----------------|-------------|
| Red WiFi (UTP) | ALTA | ALTA | ALTA | CRÍTICO |
| Broker MQTT (broker.hivemq.com) | MEDIA | ALTA | ALTA | ALTO |
| Conexión Internet | BAJA | MEDIA | ALTA | ALTO |
| API REST (puerto 8123) | ALTA | ALTA | ALTA | CRÍTICO |
| Telegram API | MEDIA | ALTA | MEDIA | MEDIO |

### **📁 Datos**

| Activo | Confidencialidad | Integridad | Disponibilidad | Importancia |
|--------|------------------|------------|----------------|-------------|
| Logs de sensores (PostgreSQL) | MEDIA | ALTA | MEDIA | ALTO |
| Eventos RFID | ALTA | ALTA | MEDIA | ALTO |
| Comandos de control | MEDIA | ALTA | ALTA | ALTO |
| Credenciales WiFi | ALTA | ALTA | BAJA | CRÍTICO |
| Token Telegram Bot | ALTA | ALTA | BAJA | CRÍTICO |
| Configuración .env | ALTA | ALTA | BAJA | CRÍTICO |

---

## ⚡ **Actividad 2 - Análisis STRIDE**

### **🎯 ESP32 Device #1 (Sensor)**

#### **S - Spoofing (Suplantación)**
- **Amenaza**: Dispositivo malicioso enviando datos falsos como "esp32-sensor-01"
- **Vector**: MQTT sin autenticación permite cualquier client_id
- **Impacto**: Datos corruptos en base de datos, decisiones erróneas

#### **T - Tampering (Manipulación)**
- **Amenaza**: Modificación del firmware por acceso físico
- **Vector**: Puerto serie/JTAG accesible, flash memory no encriptada
- **Impacto**: Comportamiento malicioso, robo de credenciales

#### **R - Repudiation (Repudio)**
- **Amenaza**: No hay forma de probar qué dispositivo envió qué datos
- **Vector**: Falta de firmas digitales/timestamps seguros
- **Impacto**: Imposible auditar eventos de seguridad

#### **I - Information Disclosure (Revelación)**
- **Amenaza**: Interceptación de tráfico MQTT con datos sensibles
- **Vector**: Comunicación sin encriptar, credenciales en código
- **Impacto**: Exposición de patrones de uso, ubicación, actividad

#### **D - Denial of Service (DoS)**
- **Amenaza**: Flooding del sistema con mensajes MQTT masivos
- **Vector**: Broker público sin rate limiting
- **Impacto**: Sistema inoperante, base de datos saturada

#### **E - Elevation of Privilege (Escalación)**
- **Amenaza**: Acceso a red WiFi empresarial a través del ESP32
- **Vector**: Credenciales WiFi almacenadas en texto plano
- **Impacto**: Acceso lateral a otros sistemas de la red

### **🎯 ESP32 Device #2 (Actuator)**

#### **S - Spoofing**
- **Amenaza**: Comandos falsos desde dispositivo suplantado
- **Vector**: Sin autenticación de origen de comandos
- **Impacto**: Activación no autorizada de actuadores

#### **T - Tampering**
- **Amenaza**: Modificación física de actuadores (LEDs, buzzer)
- **Vector**: Acceso físico sin protección
- **Impacto**: Comportamiento inesperado del sistema

#### **R - Repudiation**
- **Amenaza**: Negación de ejecución de comandos maliciosos
- **Vector**: Sin logs locales de comandos ejecutados
- **Impacto**: Imposible determinar responsabilidad

#### **I - Information Disclosure**
- **Amenaza**: Estado de LEDs revela información del sistema
- **Vector**: Observación visual del dispositivo
- **Impacto**: Revelación de patrones operacionales

#### **D - Denial of Service**
- **Amenaza**: Comando malicioso bloquea el dispositivo
- **Vector**: Sin validación de comandos, loop infinito posible
- **Impacto**: Dispositivo inoperante

#### **E - Elevation of Privilege**
- **Amenaza**: Control total del dispositivo desde comando MQTT
- **Vector**: Sin autorización/autenticación de comandos
- **Impacto**: Control completo del actuador

### **🎯 Servidor Rust**

#### **S - Spoofing**
- **Amenaza**: API calls desde clientes no autorizados
- **Vector**: Sin autenticación en endpoints REST
- **Impacto**: Acceso no autorizado a datos y funcionalidad

#### **T - Tampering**
- **Amenaza**: Inyección SQL en base de datos
- **Vector**: Posible falta de sanitización en queries
- **Impacto**: Corrupción o extracción de datos

#### **R - Repudiation**
- **Amenaza**: Acciones no atribuibles a usuarios específicos
- **Vector**: Sin sistema de autenticación/logging de usuarios
- **Impacto**: Imposible auditar acciones

#### **I - Information Disclosure**
- **Amenaza**: Exposición de datos sensibles vía API
- **Vector**: Endpoints sin control de acceso
- **Impacto**: Filtración de logs históricos, configuración

#### **D - Denial of Service**
- **Amenaza**: Sobrecarga del servidor con requests
- **Vector**: Sin rate limiting en API REST
- **Impacto**: Servidor inaccesible

#### **E - Elevation of Privilege**
- **Amenaza**: Ejecución de comandos del sistema
- **Vector**: Posible command injection en parámetros
- **Impacto**: Compromiso total del servidor

### **🎯 Bot de Telegram**

#### **S - Spoofing**
- **Amenaza**: Bot falso con token robado
- **Vector**: Token en texto plano en .env
- **Impacto**: Comandos maliciosos desde bot suplantado

#### **T - Tampering**
- **Amenaza**: Modificación de comandos en tránsito
- **Vector**: Dependiente de seguridad de Telegram
- **Impacto**: Comandos alterados

#### **R - Repudiation**
- **Amenaza**: Negación de envío de comandos por Telegram
- **Vector**: Sin firma digital de comandos
- **Impacto**: Imposible probar origen de comandos

#### **I - Information Disclosure**
- **Amenaza**: Historial de comandos visible en Telegram
- **Vector**: Chats almacenados en servidores de Telegram
- **Impacto**: Exposición de patrones de control

#### **D - Denial of Service**
- **Amenaza**: Spam masivo al bot
- **Vector**: Bot público sin rate limiting
- **Impacto**: Bot inoperante

#### **E - Elevation of Privilege**
- **Amenaza**: Acceso total al sistema vía comandos
- **Vector**: Bot con permisos completos de control
- **Impacto**: Control total desde Telegram

---

## 🚨 **Actividad 3 - Vulnerabilidades Presentes**

### **🔴 Vulnerabilidades Críticas**

#### **1. Credenciales en Texto Plano**
- **Ubicación**: `sdkconfig.defaults`, código fuente ESP32
- **Problema**: WiFi password y configuración visible
- **Por qué**: Facilidad de desarrollo, no se consideró seguridad

#### **2. MQTT sin Autenticación**
- **Ubicación**: Broker público (broker.hivemq.com)
- **Problema**: Cualquiera puede enviar/recibir mensajes
- **Por qué**: Broker gratuito público, sin configuración de seguridad

#### **3. API REST sin Autenticación**
- **Ubicación**: Servidor Rust (puerto 8123)
- **Problema**: Endpoints accesibles sin credenciales
- **Por qué**: Prototipo de desarrollo, autenticación no implementada

#### **4. Token Telegram Expuesto**
- **Ubicación**: Archivo `.env`
- **Problema**: Token en texto plano
- **Por qué**: Configuración de desarrollo, no se cifró

### **🟡 Vulnerabilidades Altas**

#### **5. Firmware ESP32 sin Encriptar**
- **Ubicación**: Flash memory de ESP32s
- **Problema**: Código fuente extraíble
- **Por qué**: ESP-IDF por defecto no encripta flash

#### **6. Sin Validación de Comandos**
- **Ubicación**: ESP32 Device #2 command parser
- **Problema**: Comandos maliciosos pueden causar DoS
- **Por qué**: Parser básico sin validación exhaustiva

#### **7. Logs sin Retención/Rotación**
- **Ubicación**: PostgreSQL database
- **Problema**: Crecimiento ilimitado de base de datos
- **Por qué**: No se implementó limpieza automática

### **🟠 Vulnerabilidades Medias**

#### **8. Tráfico MQTT en Texto Plano**
- **Ubicación**: Comunicación WiFi
- **Problema**: Datos interceptables en red local
- **Por qué**: TLS opcional no implementado por defecto

#### **9. Sin Rate Limiting**
- **Ubicación**: API REST, MQTT, Telegram Bot
- **Problema**: Susceptible a ataques de flooding
- **Por qué**: No se consideró para prototipo

#### **10. Acceso Físico sin Protección**
- **Ubicación**: Dispositivos ESP32
- **Problema**: Puertos serie accesibles
- **Por qué**: Dispositivos de desarrollo, no producción

---

## 📊 **Actividad 4 - Evaluación de Riesgo**

### **Matriz de Riesgo**

| Vulnerabilidad | Impacto | Probabilidad | Riesgo Total |
|----------------|---------|--------------|--------------|
| Credenciales en texto plano | ALTO (4) | ALTA (4) | **CRÍTICO (16)** |
| MQTT sin autenticación | ALTO (4) | ALTA (4) | **CRÍTICO (16)** |
| API sin autenticación | MEDIO (3) | ALTA (4) | **ALTO (12)** |
| Token Telegram expuesto | ALTO (4) | MEDIA (3) | **ALTO (12)** |
| Firmware sin encriptar | ALTO (4) | MEDIA (3) | **ALTO (12)** |
| Sin validación comandos | MEDIO (3) | MEDIA (3) | **MEDIO (9)** |
| Tráfico en texto plano | MEDIO (3) | MEDIA (3) | **MEDIO (9)** |
| Sin rate limiting | BAJO (2) | ALTA (4) | **MEDIO (8)** |
| Logs sin rotación | BAJO (2) | ALTA (4) | **MEDIO (8)** |
| Acceso físico | ALTO (4) | BAJA (2) | **MEDIO (8)** |

### **Escala de Evaluación**
- **Impacto**: 1=Muy Bajo, 2=Bajo, 3=Medio, 4=Alto, 5=Muy Alto
- **Probabilidad**: 1=Muy Baja, 2=Baja, 3=Media, 4=Alta, 5=Muy Alta
- **Riesgo Total**: Impacto × Probabilidad

---

## 🛡️ **Actividad 5 - Recomendaciones Finales**

### **🔴 Prioridad Crítica (Inmediato)**

#### **1. Implementar Autenticación MQTT**
```rust
// Configurar usuario/password para MQTT
let mqtt_conf = MqttClientConfiguration {
    username: Some(env::var("MQTT_USERNAME")?),
    password: Some(env::var("MQTT_PASSWORD")?),
    // ...
};
```

#### **2. Proteger Credenciales**
```bash
# Usar variables de entorno en lugar de hardcoding
export WIFI_SSID="UTP"
export WIFI_PASSWORD="tecnologica"
```

#### **3. Agregar Autenticación API**
```rust
// Middleware de autenticación con JWT
.layer(AuthLayer::new(jwt_secret))
```

### **🟡 Prioridad Alta (1-2 semanas)**

#### **4. Habilitar TLS/Encryption**
```bash
# Usar certificados generados
./security/generate_certificates.sh
```

#### **5. Implementar Rate Limiting**
```rust
.layer(RateLimitLayer::new(100, Duration::from_secs(60)))
```

#### **6. Encriptar Firmware ESP32**
```cmake
# En CMakeLists.txt
idf_build_set_property(COMPILE_OPTIONS "-DCONFIG_SECURE_FLASH_ENC_ENABLED" APPEND)
```

### **🟠 Prioridad Media (1 mes)**

#### **7. Sistema de Logging Avanzado**
```rust
// Structured logging con rotación
use tracing_appender::rolling;
let file_appender = rolling::daily("/var/log", "esp32-iot.log");
```

#### **8. Validación de Input**
```rust
// Validar todos los comandos MQTT
fn validate_command(cmd: &str) -> Result<Command, ValidationError> {
    // Implementar whitelist de comandos
}
```

#### **9. Backup y Recovery**
```sql
-- Backup automático de PostgreSQL
pg_dump esp32_iot > backup_$(date +%Y%m%d).sql
```

### **🟢 Prioridad Baja (Futuro)**

#### **10. Monitoring y Alertas**
```rust
// Métricas con Prometheus
use prometheus::{Counter, Registry};
```

#### **11. Audit Trail Completo**
```rust
// Log todas las acciones con timestamp/user
audit_log.info("Command executed", user_id, timestamp, action);
```

---

## ❓ **Consultas Adicionales**

### **1. Impacto del flooding MQTT público**

**Escenario**: Atacante publica mensajes masivos en broker.hivemq.com

**Impactos**:
- 📊 **Base de datos**: Saturación de PostgreSQL con datos falsos
- 🖥️ **Servidor**: CPU/memoria agotada procesando mensajes
- 📱 **Interfaz**: Dashboard Node-RED inoperante por sobrecarga
- 💰 **Costo**: Posible facturación excesiva si se migra a broker pagado

**Mitigación**: Rate limiting, autenticación MQTT, validación de device_id

---

### **2. Riesgo del token Telegram clonado**

**Escenario**: Usuario malicioso obtiene TELEGRAM_BOT_TOKEN

**Riesgos**:
- 🤖 **Control total**: Puede enviar cualquier comando al sistema
- 📊 **Información**: Acceso a estados y logs históricos
- 🔧 **Sabotaje**: Activación maliciosa de actuadores
- 👥 **Suplantación**: Bot falso confunde a usuarios legítimos

**Mitigación**: 
```rust
// Whitelist de chat_ids autorizados
const AUTHORIZED_USERS: &[i64] = &[123456789, 987654321];
```

---

### **3. Información inferible del tráfico MQTT**

**Datos aparentemente "simples"** revelan:

- 🏠 **Patrones de ocupación**: Horarios de actividad por temperatura/RFID
- 💡 **Hábitos**: Frecuencia de uso de LEDs indica presencia
- 🚪 **Accesos**: Eventos RFID muestran entradas/salidas
- 🌡️ **Ubicación**: Temperatura revela si hay calefacción/AC
- ⏰ **Rutinas**: Timestamps permiten mapear schedule diario

**Mitigación**: Encriptación TLS, datos agregados en lugar de raw

---

### **4. Componentes vulnerables a ingeniería social**

**Más vulnerables**:
1. 🤖 **Bot Telegram**: Usuarios pueden ser engañados para revelar comandos
2. 🌐 **Node-RED Dashboard**: Interface web sin autenticación
3. 👨‍💻 **Administrador**: Acceso a .env y credenciales
4. 📱 **Usuario final**: Puede ser manipulado para ejecutar comandos

**Menos vulnerables**:
- 🔧 **ESP32s**: Requieren acceso físico/técnico
- 🗄️ **PostgreSQL**: Backend, no expuesto directamente

**Mitigación**: Educación usuarios, autenticación multi-factor

---

### **5. Superficie de ataque de plataformas externas**

**Nuevos vectores agregados**:

#### **Telegram**:
- ☁️ **Dependencia externa**: Fallo de Telegram afecta control
- 🔐 **Modelo de confianza**: Dependes de seguridad de Telegram
- 📊 **Metadata**: Telegram puede correlacionar patrones de uso
- 🌍 **Jurisdicción**: Datos almacenados en servidores extranjeros

#### **Broker MQTT Público**:
- 🕵️ **Visibilidad**: Otros usuarios pueden monitorear tráfico
- 🔒 **Sin control**: No puedes configurar seguridad del broker
- 📈 **Escalabilidad**: Limitaciones de rate y conexiones
- ⚡ **Disponibilidad**: Sin SLA garantizado

**Mitigación**: Brokers privados, encryption end-to-end

---

### **6. Resiliencia ante caída del broker MQTT**

**Impactos de caída total**:
- 🔄 **Comunicación cruzada**: ESP32s no pueden intercambiar comandos
- 📱 **Control remoto**: Dashboard/Telegram pierden conectividad
- 📊 **Logging**: Datos de sensores no llegan al servidor
- ⚡ **Actuación**: Comandos externos no alcanzan ESP32 #2

**Resiliencia actual**: **BAJA** - Sistema centralizado en MQTT

**Mejoras sugeridas**:
```rust
// Fallback a comunicación directa HTTP
if mqtt_failed {
    send_direct_http_command(&esp32_ip, &command).await?;
}
```

**Backup local**: Broker Mosquitto local como failover

---

### **7. Amenazas por acceso físico a ESP32**

**Escenarios posibles**:

#### **Extracción de firmware**:
```bash
esptool.py read_flash 0x0 0x400000 firmware_dump.bin
```
- 📄 **Credenciales WiFi** expuestas
- 🔑 **Tokens/claves** revelados
- 📋 **Lógica del sistema** comprometida

#### **Hardware hacking**:
- 🔌 **Puerto serie**: Shell access durante boot
- ⚡ **JTAG**: Debug completo del firmware
- 🔧 **GPIO manipulation**: Control directo de pines

#### **Physical tampering**:
- 🎛️ **Sensor spoofing**: Inyectar lecturas falsas
- 💾 **Flash replacement**: Firmware malicioso
- 📡 **WiFi deauth**: Disconnect del network

**Mitigaciones**:
```c
// Secure Boot + Flash Encryption
CONFIG_SECURE_BOOT=y
CONFIG_SECURE_FLASH_ENC_ENABLED=y
```

---

### **8. Logs útiles para investigación de incidentes**

#### **🔍 Logs Críticos para Forensics**:

#### **1. Authentication/Access Logs**:
```json
{
  "timestamp": "2024-01-15T10:30:15Z",
  "source_ip": "192.168.1.100", 
  "endpoint": "/api/device_command",
  "user_agent": "curl/7.68.0",
  "success": false,
  "error": "unauthorized"
}
```

#### **2. Command Execution Logs**:
```json
{
  "timestamp": "2024-01-15T10:31:00Z",
  "from_device": "telegram-bot",
  "to_device": "esp32-actuator-01",
  "command": "LED_ON",
  "execution_status": "success",
  "response_time_ms": 245
}
```

#### **3. Anomaly Detection Logs**:
```json
{
  "timestamp": "2024-01-15T10:32:30Z",
  "anomaly_type": "unusual_frequency",
  "description": "1000+ MQTT messages in 60s",
  "source": "unknown_client_id_xyz",
  "severity": "high"
}
```

#### **4. Network Security Logs**:
```json
{
  "timestamp": "2024-01-15T10:33:15Z",
  "event_type": "connection_attempt",
  "source_ip": "192.168.1.50",
  "destination_port": 8123,
  "protocol": "HTTP",
  "blocked": true,
  "reason": "rate_limit_exceeded"
}
```

#### **5. Data Integrity Logs**:
```json
{
  "timestamp": "2024-01-15T10:34:00Z",
  "device": "esp32-sensor-01", 
  "sensor": "temperature",
  "value": 45.2,
  "checksum": "a1b2c3d4",
  "validation": "passed"
}
```

**Implementación recomendada**:
```rust
// Structured logging con serde
#[derive(Serialize)]
struct SecurityEvent {
    timestamp: DateTime<Utc>,
    event_type: String,
    source: String,
    severity: LogLevel,
    details: serde_json::Value,
}
```

---

## 📈 **Conclusión del Análisis**

El sistema actual tiene **arquitectura sólida** pero **múltiples vulnerabilidades** típicas de entornos de desarrollo. Las **prioridades críticas** se centran en autenticación y encriptación, mientras que las mejoras de **monitoring y auditoría** pueden implementarse gradualmente.

**Riesgo general actual**: 🟡 **MEDIO-ALTO**  
**Riesgo con mitigaciones**: 🟢 **BAJO-MEDIO**