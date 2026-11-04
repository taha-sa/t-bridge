//! Karşılıklı TLS: her cihazın kendi self-signed sertifikası var.
//! İstemci, sunucunun parmak izini pinler; sunucu istemci sertifikasını ister ve
//! parmak izini uygulama katmanında eşleşmiş listeyle (veya eşleşme koduyla) doğrular.
use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use quinn::{ClientConfig, ServerConfig, TransportConfig, VarInt};
use rustls::{
    DigitallySignedStruct, DistinguishedName, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::CryptoProvider,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
    server::danger::{ClientCertVerified, ClientCertVerifier},
};

use crate::{proto::ALPN, store::Identity, store::fingerprint};

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

pub fn transport() -> Arc<TransportConfig> {
    let mut t = TransportConfig::default();
    // LAN: geniş pencereler tek akışla Wi-Fi'ı doldurur; bellek üst sınırı pencere kadardır.
    t.stream_receive_window(VarInt::from_u32(8 * 1024 * 1024));
    t.receive_window(VarInt::from_u32(16 * 1024 * 1024));
    t.send_window(16 * 1024 * 1024);
    // Pil: 20 sn'de bir canlı tut; 60 sn sessizlikte bağlantı düşmüş sayılır.
    t.keep_alive_interval(Some(Duration::from_secs(20)));
    t.max_idle_timeout(Some(VarInt::from_u32(60_000).into()));
    t.datagram_receive_buffer_size(Some(256 * 1024));
    t.max_concurrent_uni_streams(VarInt::from_u32(16));
    t.max_concurrent_bidi_streams(VarInt::from_u32(4));
    Arc::new(t)
}

fn key(id: &Identity) -> PrivateKeyDer<'static> {
    PrivatePkcs8KeyDer::from(id.key_der.clone()).into()
}

fn verify_sig12(m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls12_signature(m, c, d, &provider().signature_verification_algorithms)
}

fn verify_sig13(m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls13_signature(m, c, d, &provider().signature_verification_algorithms)
}

fn schemes() -> Vec<SignatureScheme> {
    provider().signature_verification_algorithms.supported_schemes()
}

/// Sunucu sertifikasını sadece beklenen parmak iziyle kabul eder.
#[derive(Debug)]
struct PinnedServer {
    expected: String,
}

impl ServerCertVerifier for PinnedServer {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if fingerprint(end_entity) == self.expected {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("sunucu parmak izi eşleşmiyor".into()))
        }
    }
    fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_sig12(m, c, d)
    }
    fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_sig13(m, c, d)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        schemes()
    }
}

/// İstemci sertifikasını zorunlu kılar; imzayı doğrular, kimliği uygulama katmanı denetler.
#[derive(Debug)]
struct AnyClient;

impl ClientCertVerifier for AnyClient {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }
    fn verify_client_cert(&self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: UnixTime) -> Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_sig12(m, c, d)
    }
    fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_sig13(m, c, d)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        schemes()
    }
    fn client_auth_mandatory(&self) -> bool {
        true
    }
}

pub fn server_config(id: &Identity) -> Result<ServerConfig> {
    let mut tls = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_client_cert_verifier(Arc::new(AnyClient))
        .with_single_cert(vec![CertificateDer::from(id.cert_der.clone())], key(id))
        .context("sunucu sertifikası")?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let mut sc = ServerConfig::with_crypto(Arc::new(quinn::crypto::rustls::QuicServerConfig::try_from(tls)?));
    sc.transport_config(transport());
    Ok(sc)
}

pub fn client_config(id: &Identity, server_fp: &str) -> Result<ClientConfig> {
    let mut tls = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedServer { expected: server_fp.to_string() }))
        .with_client_auth_cert(vec![CertificateDer::from(id.cert_der.clone())], key(id))
        .context("istemci sertifikası")?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let mut cc = ClientConfig::new(Arc::new(quinn::crypto::rustls::QuicClientConfig::try_from(tls)?));
    cc.transport_config(transport());
    Ok(cc)
}

/// Bağlantıdaki karşı taraf sertifikasının parmak izi.
pub fn peer_fingerprint(conn: &quinn::Connection) -> Option<String> {
    let any = conn.peer_identity()?;
    let certs = any.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    certs.first().map(|c| fingerprint(c))
}
