use crate::error::CommandError;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;

#[derive(Copy, Clone)]
pub enum Command {
    PanelSetting = 0x00,
    PowerSetting = 0x01,
    PowerOff = 0x02,
    PowerOffSequenceSettings = 0x03,
    PowerOn = 0x04,
    PowerOnMeasure = 0x05,
    BoosterSoftStart = 0x06,
    DeepSleep = 0x07,
    DisplayStartTransmission1 = 0x10,
    DataStop = 0x11,
    DisplayRefresh = 0x12,
    DisplayStartTransmission2 = 0x13,
    AutoSequence = 0x17,
    VCOMLut = 0x20,
    W2WLut = 0x21,
    KW2Lut = 0x22,
    W2KLut = 0x23,
    K2KLut = 0x24,
    LutOption = 0x2A,
    PLLControl = 0x30,
    TemperatureSensorCalibration = 0x40,
    TemperatureSensorSelection = 0x41,
    TemperatureSensorWrite = 0x42,
    TemperatureSensorRead = 0x43,
    PanelBreakCheck = 0x44,
    VCOMAndDataIntervalSetting = 0x50,
    LowPowerDetection = 0x51,
    TCONSetting = 0x60,
    ResolutionSetting = 0x61,
    GateSourceStartSetting = 0x65,
    Revision = 0x70,
    GetStatus = 0x71,
    CyclicRedundancyCheck = 0x72,
    AutoMeasurementVCOM = 0x80,
    VCOMDCSetting = 0x82,
    PartialWindow = 0x90,
    PartialIn = 0x91,
    PartialOut = 0x92,
    ProgramMode = 0xA0,
    ActiveProgramming = 0xA1,
    CascadeSetting = 0xE0,
    PowerSaving = 0xE3,
    LVDVoltageSelect = 0xE4,
    ForceTemperature = 0xE5,
    ReadVCOMValue = 0x81,
    ReadOTP = 0xA2,
}

impl Command {
    pub fn register(&self) -> u8 {
        *self as u8
    }

    pub async fn send<SPI: SpiDevice, DC: OutputPin>(
        &self,
        spi: &mut SPI,
        dc: &mut DC,
    ) -> Result<(), CommandError<SPI::Error, DC::Error>> {
        debug!("command {:X} sent", self.register());

        dc.set_low().map_err(CommandError::DC)?;
        spi.write(&[self.register()])
            .await
            .map_err(CommandError::SPI)?;

        Ok(())
    }

    pub async fn send_with_data<SPI: SpiDevice, DC: OutputPin>(
        &self,
        spi: &mut SPI,
        dc: &mut DC,
        data: &[u8],
    ) -> Result<(), CommandError<SPI::Error, DC::Error>> {
        debug!("command {:x} sent", self.register());

        dc.set_low().map_err(CommandError::DC)?;
        spi.write(&[self.register()])
            .await
            .map_err(CommandError::SPI)?;
        if !data.is_empty() {
            dc.set_high().map_err(CommandError::DC)?;
            spi.write(data).await.map_err(CommandError::SPI)?;
            if data.len() <= 10 {
                debug!("command data sent {:x}", data)
            } else {
                if data.iter().any(|&p| p == 0x00) {
                    debug!("command data sent, array of 0x00, count: {}", data.len())
                } else if data.iter().any(|&p| p == 0xFF) {
                    debug!("command data sent, array of 0xFF, count: {}", data.len())
                } else {
                    debug!(
                        "command data sent, array of mixed values, count: {}",
                        data.len()
                    )
                }
            }
        }

        Ok(())
    }
}
