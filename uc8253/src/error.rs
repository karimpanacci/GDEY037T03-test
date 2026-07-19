//! Currently this module is work in progress.

#[derive(Debug)]
pub enum Error<SPIError, DCError, RSTError, BSYError> {
    SPI(SPIError),
    DC(DCError),
    RST(RSTError),
    BSY(BSYError),
}

impl<SPIError, DCError, RSTError, BSYError> From<CommandError<SPIError, DCError>>
    for Error<SPIError, DCError, RSTError, BSYError>
{
    fn from(value: CommandError<SPIError, DCError>) -> Self {
        match value {
            CommandError::SPI(e) => Self::SPI(e),
            CommandError::DC(e) => Self::DC(e),
        }
    }
}

#[derive(Debug)]
pub enum CommandError<SPIError, DCError> {
    SPI(SPIError),
    DC(DCError),
}

impl<SPIError, DCError> CommandError<SPIError, DCError> {
    pub fn to_error<RSTError, BSYError>(self) -> Error<SPIError, DCError, RSTError, BSYError> {
        self.into()
    }
}
