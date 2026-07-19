//! Simple SPI driver for the GDEH0154D67 E-Paper display.
//! This crate is a `no_std` library that provides an interface compatible with [embedded-hal-1.0.0-rc.1](https://docs.rs/embedded-hal/1.0.0-rc.1/embedded_hal/).
//! It is also designed to be used together with [embedded-graphics](https://docs.rs/embedded-graphics/latest/embedded_graphics/).
//! It ensures a correct initialization and a consistent state at every moment by enforcing design
//! constrains at compile time using zero cost abstractions.
//!
//! The crate has a `std` feature for use in fully `std` environments, the only effect of which is that [`error::Error`] implements `std:error::Error`.
//! There is also the `heap_buffer` feature, which allocates the internal graphics buffer on the heap, preventing stack overflows on more limited platorms.
//! This feature of course requires an allocator.
#![no_std]

#[macro_use]
mod fmt;
pub mod command;
pub mod error;

use error::Error;

use embedded_graphics::{
    draw_target::DrawTarget, geometry::OriginDimensions, geometry::Size, pixelcolor::BinaryColor,
    Pixel,
};

use crate::command::Command;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::{digital::Wait, spi::SpiDevice};

pub struct UC8253<SPI, DC, RST, BSY, DLY, S> {
    interface: SPI,
    old_buffer: Buffer,
    new_buffer: Buffer,
    dc: DC,
    reset: RST,
    busy: BSY,
    delay: DLY,
    #[allow(dead_code)]
    state: S,
}

/// Sets the display as initialized, only after calling the method `init()`
/// the display is considered initialized.
pub struct Initialized;
/// Sets the display as not initialized. This state occurs when acquiring
/// a new instance of `GDEH0154D67` or after doing a `reset()` of the display.
pub struct NotInitialized;

pub const PIXELS_X: usize = 240;
pub const PIXELS_Y: usize = 416;
const PIXELS_ARRAY: usize = PIXELS_X * PIXELS_Y / 8;
/// Struct that stores the binary color of the pixels that will be set when the
/// next display update is performed
#[derive(Clone)]
struct Buffer {
    pixels: [u8; PIXELS_X * PIXELS_Y / 8],
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RefreshMode {
    Full,
    Fast,
    Partial,
}

impl<SPI, DC, RST, BSY, DLY> UC8253<SPI, DC, RST, BSY, DLY, NotInitialized>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BSY: InputPin + Wait,
    DLY: DelayNs,
{
    /// Acquires the SPI interface and the control GPIO pins. It also performs
    /// a hardware reset on the device.
    pub fn new(interface: SPI, dc: DC, reset: RST, busy: BSY, delay: DLY) -> Self {
        Self {
            interface,
            old_buffer: Buffer { pixels: [0; _] },
            new_buffer: Buffer { pixels: [0; _] },
            dc,
            reset,
            busy,
            delay,
            state: NotInitialized,
        }
    }

    /// Releases SPI interface and control pins.
    pub fn release(
        self,
    ) -> Result<(SPI, DC, RST, BSY), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        Ok((self.interface, self.dc, self.reset, self.busy))
    }

    /// Sets the display into an initialized state. It is required to call this
    /// method once before any display update can be done.
    pub async fn init(
        mut self,
        refresh_mode: RefreshMode,
    ) -> Result<
        UC8253<SPI, DC, RST, BSY, DLY, Initialized>,
        Error<SPI::Error, DC::Error, RST::Error, BSY::Error>,
    > {
        self.reset().await?;

        match refresh_mode {
            RefreshMode::Full => self.full_refresh_init().await?,
            RefreshMode::Fast => self.fast_refresh_init().await?,
            RefreshMode::Partial => self.partial_refresh_init().await?,
        }

        Ok(UC8253 {
            interface: self.interface,
            new_buffer: self.new_buffer,
            old_buffer: self.old_buffer,
            dc: self.dc,
            reset: self.reset,
            busy: self.busy,
            delay: self.delay,
            state: Initialized,
        })
    }
}

impl<SPI, DC, RST, BSY, DLY> UC8253<SPI, DC, RST, BSY, DLY, Initialized>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BSY: InputPin + Wait,
    DLY: DelayNs,
{
    pub async fn write_framebuffer(
        &mut self,
    ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        self.busy_block().await?;
        Command::DisplayStartTransmission1
            .send_with_data(&mut self.interface, &mut self.dc, &self.old_buffer.pixels)
            .await?;

        Command::DisplayStartTransmission2
            .send_with_data(&mut self.interface, &mut self.dc, &self.new_buffer.pixels)
            .await?;

        self.old_buffer = self.new_buffer.clone();

        Ok(())
    }
    pub async fn update_display(
        &mut self,
    ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        self.send(Command::DisplayRefresh).await?;
        self.delay.delay_ms(1).await;
        info!("sended drf");
        Ok(())
    }

    /// Releases SPI interface and control pins.
    pub async fn release(
        self,
    ) -> Result<(SPI, DC, RST, BSY), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        let display = self.turn_off().await?;
        Ok((display.interface, display.dc, display.reset, display.busy))
    }
}

impl<SPI, DC, RST, BSY, DLY, S> UC8253<SPI, DC, RST, BSY, DLY, S>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BSY: InputPin + Wait,
    DLY: DelayNs,
{
    async fn full_refresh_init(
        &mut self,
    ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        self.send(Command::PowerOn).await?;
        self.busy_block().await?;
        self.send_with_data(Command::VCOMAndDataIntervalSetting, &[0x97])
            .await?;

        Ok(())
    }
    async fn fast_refresh_init(
        &mut self,
    ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        self.send(Command::PowerOn).await?;
        self.busy_block().await?;

        self.send_with_data(Command::CascadeSetting, &[0x02])
            .await?;
        self.send_with_data(Command::ForceTemperature, &[0x5F])
            .await?;

        Ok(())
    }
    async fn partial_refresh_init(
        &mut self,
    ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        self.send(Command::PowerOn).await?;
        self.busy_block().await?;

        self.send_with_data(Command::CascadeSetting, &[0x02])
            .await?;
        self.send_with_data(Command::ForceTemperature, &[0x6E])
            .await?;

        self.send_with_data(Command::VCOMAndDataIntervalSetting, &[0xD7])
            .await?;

        Ok(())
    }

    // pub async fn set_refresh_mode(
    //     &mut self,
    //     refresh_mode: RefreshMode,
    // ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
    //     if self.refresh_mode != refresh_mode {
    //         self.refresh_mode_impl(refresh_mode).await?;
    //     }
    //
    //     Ok(())
    // }

    // async fn refresh_mode_impl(
    //     &mut self,
    //     refresh_mode: RefreshMode,
    // ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
    //     //self.soft_reset().await?;
    //
    //     if let RefreshMode::Gray2 = refresh_mode {
    //         self.send_with_data(Command::PanelSetting, &[0x1F]).await?;
    //     }
    //
    //     let force_temperature_data = match refresh_mode {
    //         RefreshMode::Fast => Some(0x5F),
    //         RefreshMode::Partial => Some(0x6E),
    //         RefreshMode::Gray2 => Some(0x5A),
    //         _ => None,
    //     };
    //
    //     if let Some(data) = force_temperature_data {
    //         self.send_with_data(Command::CascadeSetting, &[0x02])
    //             .await?;
    //         self.send_with_data(Command::ForceTemperature, &[data])
    //             .await?;
    //     }
    //
    //     let vcom_data = match refresh_mode {
    //         RefreshMode::Full => Some(0x97),
    //         RefreshMode::Partial => Some(0xD7),
    //         _ => None,
    //     };
    //
    //     if let Some(data) = vcom_data {
    //         self.send_with_data(Command::VCOMAndDataIntervalSetting, &[data])
    //             .await?;
    //     }
    //
    //     Ok(())
    // }

    /// Utility method to send command without to specify SPI and DC.
    /// It will wait ready signal before send command
    pub async fn send_with_data(
        &mut self,
        command: Command,
        data: &[u8],
    ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        self.busy_block().await?;
        command
            .send_with_data(&mut self.interface, &mut self.dc, data)
            .await?;
        Ok(())
    }
    /// Utility method to send command without to specify SPI and DC.
    /// It will wait ready signal before send command
    pub async fn send(
        &mut self,
        command: Command,
    ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        self.busy_block().await?;
        command.send(&mut self.interface, &mut self.dc).await?;

        Ok(())
    }

    /// Sends the display into deep sleep mode.
    pub async fn turn_off(
        mut self,
    ) -> Result<
        UC8253<SPI, DC, RST, BSY, DLY, NotInitialized>,
        Error<SPI::Error, DC::Error, RST::Error, BSY::Error>,
    > {
        self.send(Command::PowerOff).await?;
        self.send_with_data(Command::DeepSleep, &[0xA5]).await?;

        // Ensure all the pin is off to avoid leakage current
        self.dc.set_low().map_err(Error::DC)?;

        Ok(UC8253 {
            interface: self.interface,
            old_buffer: self.old_buffer,
            new_buffer: self.new_buffer,
            dc: self.dc,
            reset: self.reset,
            busy: self.busy,
            delay: self.delay,
            state: NotInitialized,
        })
    }

    /// Performs a hardware reset of the display.
    async fn reset(&mut self) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        self.reset.set_low().map_err(Error::RST)?;
        self.delay.delay_ms(10).await;
        self.reset.set_high().map_err(Error::RST)?;
        self.delay.delay_ms(10).await;
        Ok(())
    }

    async fn soft_reset(
        &mut self,
    ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        self.send_with_data(Command::PanelSetting, &[0x1]).await
    }

    /// Blocks while the display is updating.
    async fn busy_block(
        &mut self,
    ) -> Result<(), Error<SPI::Error, DC::Error, RST::Error, BSY::Error>> {
        //debug!("wait for busy");

        // IT LOOKS THAT BUSY SIGNAL NEVER GOES HIGH AND THE PROGRAM STOP HERE; WE CAN BYPASS IT BY WAITING 1000MS
        self.busy.wait_for_high().await.map_err(Error::BSY)?;
        // self.delay.delay_ms(1000).await;

        //debug!("busy unlocked");

        Ok(())
    }
}

impl<SPI, DC, RST, BSY, DLY> DrawTarget for UC8253<SPI, DC, RST, BSY, DLY, Initialized>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BSY: InputPin,
    DLY: DelayNs,
{
    type Color = BinaryColor;
    type Error = Error<SPI::Error, DC::Error, RST::Error, BSY::Error>;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            // Check if the pixel coordinates are out of bounds (negative or greater than
            // (PIXELS_X, PIXELS_Y)). `DrawTarget` implementation are required to discard any out of bounds
            // pixels without returning an error or causing a panic.
            const MAX_X_IDX: u32 = PIXELS_X as u32 - 1;
            const MAX_Y_IDX: u32 = PIXELS_Y as u32 - 1;
            if let Ok((x @ 0..=MAX_X_IDX, y @ 0..=MAX_Y_IDX)) = coord.try_into() {
                // Calculate the index in the buffer.
                let index = pixel2buffer(x, y);
                let bit_index = index % 8;
                let mask = 0b10000000 >> bit_index;
                let byte_index = index / 8;
                let color_val_u8 = u8::from(color.is_on()) << 7;

                self.new_buffer.pixels[byte_index] =
                    (self.new_buffer.pixels[byte_index] & !mask) | color_val_u8 >> bit_index;
            }
        }

        Ok(())
    }
}

impl<SPI, DC, RST, BSY, DLY> OriginDimensions for UC8253<SPI, DC, RST, BSY, DLY, Initialized> {
    fn size(&self) -> Size {
        Size::new(PIXELS_X as u32, PIXELS_Y as u32)
    }
}

fn pixel2buffer(x: u32, y: u32) -> usize {
    (x + y * PIXELS_X as u32).try_into().unwrap()
}

fn index2address(i: usize) -> (u8, u8) {
    (
        (i % (PIXELS_Y / 8)).try_into().unwrap(),
        (i / (PIXELS_Y / 8)).try_into().unwrap(),
    )
}
