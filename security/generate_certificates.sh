#!/bin/bash

# 🔒 Script para generar certificados TLS para ESP32 IoT System
# Ejecutar: chmod +x generate_certificates.sh && ./generate_certificates.sh

echo "🔐 Generando certificados TLS para sistema ESP32 IoT..."

# Crear directorio para certificados
mkdir -p certs
cd certs

# 1. Generar CA (Certificate Authority)
echo "📋 Generando Certificate Authority (CA)..."
openssl genrsa -out ca.key 4096
openssl req -new -x509 -key ca.key -sha256 -subj "/C=CO/ST=Risaralda/L=Pereira/O=ESP32-IoT/OU=Security/CN=ESP32-IoT-CA" -days 3650 -out ca.crt

# 2. Generar certificado para servidor MQTT/API
echo "🖥️  Generando certificado del servidor..."
openssl genrsa -out server.key 4096
openssl req -new -key server.key -subj "/C=CO/ST=Risaralda/L=Pereira/O=ESP32-IoT/OU=Server/CN=esp32-iot-server" -out server.csr

# Extensiones para servidor
cat > server.ext << EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = esp32-iot-server
IP.1 = 127.0.0.1
IP.2 = 192.168.1.100
EOF

openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 365 -sha256 -extfile server.ext

# 3. Generar certificados para ESP32 Device #1
echo "🔧 Generando certificado ESP32 Device #1..."
openssl genrsa -out esp32-device-1.key 2048
openssl req -new -key esp32-device-1.key -subj "/C=CO/ST=Risaralda/L=Pereira/O=ESP32-IoT/OU=Device/CN=esp32-sensor-01" -out esp32-device-1.csr
openssl x509 -req -in esp32-device-1.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out esp32-device-1.crt -days 365 -sha256

# 4. Generar certificados para ESP32 Device #2  
echo "🔧 Generando certificado ESP32 Device #2..."
openssl genrsa -out esp32-device-2.key 2048
openssl req -new -key esp32-device-2.key -subj "/C=CO/ST=Risaralda/L=Pereira/O=ESP32-IoT/OU=Device/CN=esp32-actuator-01" -out esp32-device-2.csr
openssl x509 -req -in esp32-device-2.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out esp32-device-2.crt -days 365 -sha256

# 5. Convertir certificados a formato DER para ESP32
echo "🔄 Convirtiendo certificados a formato DER..."
openssl x509 -outform der -in ca.crt -out ca.der
openssl x509 -outform der -in esp32-device-1.crt -out esp32-device-1.der
openssl x509 -outform der -in esp32-device-2.crt -out esp32-device-2.der
openssl rsa -in esp32-device-1.key -outform der -out esp32-device-1.der.key
openssl rsa -in esp32-device-2.key -outform der -out esp32-device-2.der.key

# 6. Crear archivos C header para ESP32
echo "📄 Generando archivos header para ESP32..."

# CA Certificate header
echo "🔗 Creando ca_cert.h..."
cat > ca_cert.h << EOF
#ifndef CA_CERT_H
#define CA_CERT_H

extern const uint8_t ca_cert_der_start[] asm("_binary_ca_der_start");
extern const uint8_t ca_cert_der_end[] asm("_binary_ca_der_end");

#endif
EOF

# ESP32 Device 1 certificate header
echo "🔗 Creando device1_cert.h..."
cat > device1_cert.h << EOF
#ifndef DEVICE1_CERT_H  
#define DEVICE1_CERT_H

extern const uint8_t device1_cert_der_start[] asm("_binary_esp32_device_1_der_start");
extern const uint8_t device1_cert_der_end[] asm("_binary_esp32_device_1_der_end");
extern const uint8_t device1_key_der_start[] asm("_binary_esp32_device_1_der_key_start");
extern const uint8_t device1_key_der_end[] asm("_binary_esp32_device_1_der_key_end");

#endif
EOF

# ESP32 Device 2 certificate header
echo "🔗 Creando device2_cert.h..."
cat > device2_cert.h << EOF
#ifndef DEVICE2_CERT_H
#define DEVICE2_CERT_H

extern const uint8_t device2_cert_der_start[] asm("_binary_esp32_device_2_der_start");
extern const uint8_t device2_cert_der_end[] asm("_binary_esp32_device_2_der_end");
extern const uint8_t device2_key_der_start[] asm("_binary_esp32_device_2_der_key_start");
extern const uint8_t device2_key_der_end[] asm("_binary_esp32_device_2_der_key_end");

#endif
EOF

# 7. Configuración para Mosquitto MQTT Broker local
echo "🦟 Generando configuración para Mosquitto..."
cat > mosquitto.conf << EOF
# Configuración Mosquitto con TLS
listener 1883
allow_anonymous true

listener 8883
cafile $PWD/ca.crt
certfile $PWD/server.crt  
keyfile $PWD/server.key
require_certificate true
use_identity_as_username true

log_dest stdout
log_type all
EOF

# 8. Script para copiar certificados a proyectos ESP32
cat > copy_certs_to_esp32.sh << 'EOF'
#!/bin/bash

echo "📋 Copiando certificados a proyectos ESP32..."

# Crear directorios de certificados en proyectos ESP32
mkdir -p ../../esp32-device-1/main/certs
mkdir -p ../../esp32-device-2/main/certs

# Copiar certificados CA y de dispositivos
cp ca.der ../../esp32-device-1/main/certs/
cp esp32-device-1.der ../../esp32-device-1/main/certs/
cp esp32-device-1.der.key ../../esp32-device-1/main/certs/
cp ca_cert.h ../../esp32-device-1/main/
cp device1_cert.h ../../esp32-device-1/main/

cp ca.der ../../esp32-device-2/main/certs/
cp esp32-device-2.der ../../esp32-device-2/main/certs/
cp esp32-device-2.der.key ../../esp32-device-2/main/certs/
cp ca_cert.h ../../esp32-device-2/main/
cp device2_cert.h ../../esp32-device-2/main/

echo "✅ Certificados copiados a proyectos ESP32"
echo "📝 Recuerda actualizar CMakeLists.txt para incluir los archivos .der"
EOF

chmod +x copy_certs_to_esp32.sh

# 9. Limpiar archivos temporales
rm -f *.csr *.ext

# 10. Resumen de archivos generados
echo ""
echo "✅ Certificados TLS generados exitosamente!"
echo ""
echo "📁 Archivos generados:"
echo "   • ca.crt / ca.key - Certificate Authority"
echo "   • server.crt / server.key - Certificado del servidor"  
echo "   • esp32-device-1.crt / esp32-device-1.key - Device #1"
echo "   • esp32-device-2.crt / esp32-device-2.key - Device #2"
echo "   • *.der - Formato binario para ESP32"
echo "   • *.h - Headers para incluir en código C"
echo "   • mosquitto.conf - Configuración MQTT broker"
echo ""
echo "🔧 Siguientes pasos:"
echo "   1. Ejecutar: ./copy_certs_to_esp32.sh"
echo "   2. Actualizar código ESP32 para usar TLS"
echo "   3. Configurar servidor Rust con certificados"
echo "   4. Ejecutar Mosquitto: mosquitto -c mosquitto.conf"
echo ""
echo "🔒 Sistema de seguridad TLS configurado!"