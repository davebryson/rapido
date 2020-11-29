pub trait GenesisConfig {
    type Config;

    fn add(&mut self, items: Self::Config);
}

struct Example {
    data: Vec<u16>,
}

impl Example {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
}

impl GenesisConfig for Example {
    type Config = u16;

    fn add(&mut self, items: Self::Config) {
        self.data.push(items)
    }
}
