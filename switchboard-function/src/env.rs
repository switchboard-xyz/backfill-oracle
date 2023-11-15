use crate::*;
use serde::Deserialize;
use switchboard_solana::solana_sdk::signature::read_keypair_file;
use std::io::{ Read, Write };
use std::sync::OnceLock;

// A static variable representing the different node metrics to collect
pub static WORKER_ENVIRONMENT: OnceLock<WorkerEnvironment> = OnceLock::new();

fn default_rpc_url() -> String {
    "https://api.devnet.solana.com".to_string()
}

fn default_program_id() -> String {
    ProgramID.to_string()
}

fn default_pyth_rpc_url() -> String {
    "https://hermes.pyth.network".to_string()
}

#[derive(Deserialize, Debug, Default)]
pub struct WorkerEnvironment {
    #[serde(default = "default_rpc_url")]
    pub rpc_url: String,
    #[serde(default)]
    pub payer_secret: Vec<u8>,
    #[serde(default)]
    pub fs_payer_secret_path: String,
    #[serde(default = "default_program_id")]
    pub program_id: String,
    #[serde(default = "default_pyth_rpc_url")]
    pub pyth_rpc_url: String,
}
impl WorkerEnvironment {
    pub fn get_or_init() -> &'static Self {
        WORKER_ENVIRONMENT.get_or_init(|| WorkerEnvironment::parse().unwrap())
    }

    pub fn parse() -> Result<Self, SbError> {
        match envy::from_env::<WorkerEnvironment>() {
            Ok(env) => Ok(env),
            Err(error) =>
                Err(
                    SbError::CustomMessage(
                        format!("failed to decode environment variables: {}", error)
                    )
                ),
        }
    }

    /// Load the enclave signer to sign all transactions. If the keypair was generated from the enclave
    /// and never intentionally leaked outside the enclave, then we can be fairly confident the transactions
    /// were generated inside of an enclave.
    ///
    /// NOTE: There is an escape hatch to help debug and iterate locally. This should be removed for production.
    pub fn load_enclave_signer(&self, keypair_path: Option<&str>) -> Result<Arc<Keypair>, SbError> {
        let keypair_path = keypair_path.unwrap_or("/data/protected_files/keypair.bin");
        let file = std::fs::OpenOptions
            ::new()
            .read(true)
            .write(true)
            .create(true)
            .open(keypair_path);
        if file.is_err() {
            println!(
                "Keypair file was unable to be opened, likely encrypted by a different enclave signature or running outside of an enclave."
            );
            println!("Generating a fresh keypair ...");
            return Ok(Arc::new(Keypair::new()));
        }

        let mut file = file.unwrap();
        let mut sealed_buffer_vec: Vec<u8> = Vec::with_capacity(64);
        let secured_signer: Keypair;
        let res = file.read_to_end(&mut sealed_buffer_vec);
        if res.is_ok() && sealed_buffer_vec.len() == 64 {
            println!("Secured signer already exists, loading..");
            secured_signer = Keypair::from_bytes(&sealed_buffer_vec).unwrap();
        } else {
            println!("Existing secured signer not found. Creating..");
            let mut seed = [0u8; 32];
            switchboard_solana::Gramine::read_rand(&mut seed)?;
            secured_signer = keypair_from_seed(&seed).unwrap();
            file.write_all(&secured_signer.to_bytes()[..64]).unwrap();
            drop(file);
        }
        Ok(Arc::new(secured_signer))
    }

    pub fn get_payer(&self) -> Result<Arc<Keypair>, SbError> {
        if !self.payer_secret.is_empty() && self.payer_secret.len() == 64 {
            let kp = Keypair::from_bytes(&self.payer_secret[..]).map_err(
                |_| SbError::InvalidKeypairFile
            )?;
            return Ok(Arc::new(kp));
        }

        if !self.fs_payer_secret_path.is_empty() {
            let kp = read_keypair_file(self.fs_payer_secret_path.clone()).map_err(
                |_| SbError::InvalidKeypairFile
            )?;
            return Ok(Arc::new(kp));
        }

        Err(
            SbError::Message("Must provide PAYER_SECRET or FS_PAYER_SECRET_PATH to load the worker")
        )
    }

    pub fn get_program_id(&self) -> Pubkey {
        if !self.program_id.is_empty() {
            match Pubkey::from_str(&self.program_id) {
                Ok(pubkey) => {
                    return pubkey;
                }
                Err(e) => {
                    error!("Failed to decode program_id env variable: {:?}", e);
                }
            }
        }

        ProgramID
    }
}
