use crate::track::*;
use proc_macro2::TokenStream;
use quote::{ToTokens, TokenStreamExt, quote};

fn prefix() -> TokenStream {
    quote!(::eb_agb_psg_controller::__private)
}

fn interop() -> TokenStream {
    let p = prefix();
    quote!(#p::eb_agb_psg_interop)
}

fn option<T: ToTokens>(value: &Option<T>) -> TokenStream {
    match value {
        Some(v) => quote!(Some(#v)),
        None => quote!(None),
    }
}

impl ToTokens for EnvelopeSpec {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let i = interop();
        let EnvelopeSpec {
            initial_volume,
            increasing,
            step_time,
        } = self;
        tokens.append_all(quote! {
            #i::EnvelopeSpec {
                initial_volume: #initial_volume,
                increasing: #increasing,
                step_time: #step_time,
            }
        });
    }
}

impl ToTokens for SweepSpec {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let i = interop();
        let SweepSpec {
            time,
            decreasing,
            shift,
        } = self;
        tokens.append_all(quote! {
            #i::SweepSpec {
                time: #time,
                decreasing: #decreasing,
                shift: #shift,
            }
        });
    }
}

impl ToTokens for Instrument {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let i = interop();
        tokens.append_all(match self {
            Instrument::Square {
                duty,
                envelope,
                sweep,
                length,
            } => {
                let sweep = option(sweep);
                let length = option(length);
                quote! {
                    #i::Instrument::Square {
                        duty: #duty,
                        envelope: #envelope,
                        sweep: #sweep,
                        length: #length,
                    }
                }
            }
            Instrument::Wave { wave_table, volume } => quote! {
                #i::Instrument::Wave { wave_table: #wave_table, volume: #volume }
            },
            Instrument::Noise {
                envelope,
                short_lfsr,
            } => quote! {
                #i::Instrument::Noise { envelope: #envelope, short_lfsr: #short_lfsr }
            },
        });
    }
}

impl ToTokens for PsgEffect {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let p = prefix();
        let i = interop();
        tokens.append_all(match self {
            PsgEffect::None => quote!(#i::PsgEffect::None),
            PsgEffect::Arpeggio(x, y) => quote!(#i::PsgEffect::Arpeggio(#x, #y)),
            PsgEffect::PortamentoUp(v) => quote!(#i::PsgEffect::PortamentoUp(#v)),
            PsgEffect::PortamentoDown(v) => quote!(#i::PsgEffect::PortamentoDown(#v)),
            PsgEffect::TonePortamento(v) => quote!(#i::PsgEffect::TonePortamento(#v)),
            PsgEffect::Vibrato { speed, depth } => {
                quote!(#i::PsgEffect::Vibrato { speed: #speed, depth: #depth })
            }
            PsgEffect::VolumeSlide(v) => quote!(#i::PsgEffect::VolumeSlide(#v)),
            PsgEffect::NoteCut(t) => quote!(#i::PsgEffect::NoteCut(#t)),
            PsgEffect::NoteDelay(t) => quote!(#i::PsgEffect::NoteDelay(#t)),
            PsgEffect::PositionJump(t) => quote!(#i::PsgEffect::PositionJump(#t)),
            PsgEffect::PatternBreak(r) => quote!(#i::PsgEffect::PatternBreak(#r)),
            PsgEffect::SetTicksPerRow(t) => quote!(#i::PsgEffect::SetTicksPerRow(#t)),
            PsgEffect::SetFramesPerTick(f) => {
                let raw = f.to_raw();
                quote!(#i::PsgEffect::SetFramesPerTick(#p::Num::from_raw(#raw)))
            }
            PsgEffect::SetDuty(d) => quote!(#i::PsgEffect::SetDuty(#d)),
            PsgEffect::SetPan { left, right } => {
                quote!(#i::PsgEffect::SetPan { left: #left, right: #right })
            }
            PsgEffect::SetVolume(v) => quote!(#i::PsgEffect::SetVolume(#v)),
        });
    }
}

impl ToTokens for PatternSlot {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let i = interop();
        let PatternSlot {
            note,
            instrument,
            effect,
        } = self;
        tokens.append_all(quote! {
            #i::PatternSlot { note: #note, instrument: #instrument, effect: #effect }
        });
    }
}

impl ToTokens for Pattern {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let i = interop();
        let Pattern {
            start_row,
            num_rows,
        } = self;
        tokens.append_all(quote! {
            #i::Pattern { start_row: #start_row, num_rows: #num_rows }
        });
    }
}

impl ToTokens for SfxChannel {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let i = interop();
        tokens.append_all(match self {
            SfxChannel::SquareSweep => quote!(#i::SfxChannel::SquareSweep),
            SfxChannel::Square => quote!(#i::SfxChannel::Square),
            SfxChannel::Wave => quote!(#i::SfxChannel::Wave),
            SfxChannel::Noise => quote!(#i::SfxChannel::Noise),
        });
    }
}

fn wave_tables_tokens(tables: &[[u8; 16]]) -> TokenStream {
    let tables = tables.iter().map(|table| {
        let bytes = table.iter();
        quote!([#(#bytes),*])
    });
    quote!(&[#(#tables),*])
}

impl ToTokens for Track {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let p = prefix();
        let i = interop();
        let instruments = self.instruments.iter();
        let wave_tables = wave_tables_tokens(&self.wave_tables);
        let pattern_data = self.pattern_data.iter();
        let patterns = self.patterns.iter();
        let pattern_order = self.pattern_order.iter();
        let frames_per_tick = self.frames_per_tick.to_raw();
        let ticks_per_row = self.ticks_per_row;
        let loop_order_index = option(&self.loop_order_index);
        tokens.append_all(quote! {{
            static INSTRUMENTS: &[#i::Instrument] = &[#(#instruments),*];
            static WAVE_TABLES: &[[u8; 16]] = #wave_tables;
            static PATTERN_DATA: &[#i::PatternSlot] = &[#(#pattern_data),*];
            static PATTERNS: &[#i::Pattern] = &[#(#patterns),*];
            static PATTERN_ORDER: &[u8] = &[#(#pattern_order),*];
            #i::Track {
                instruments: #p::Cow::Borrowed(INSTRUMENTS),
                wave_tables: #p::Cow::Borrowed(WAVE_TABLES),
                pattern_data: #p::Cow::Borrowed(PATTERN_DATA),
                patterns: #p::Cow::Borrowed(PATTERNS),
                pattern_order: #p::Cow::Borrowed(PATTERN_ORDER),
                frames_per_tick: #i::FrameCount::from_raw(#frames_per_tick),
                ticks_per_row: #ticks_per_row,
                loop_order_index: #loop_order_index,
            }
        }});
    }
}

impl ToTokens for Sfx {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let p = prefix();
        let i = interop();
        let channel = &self.channel;
        let instruments = self.instruments.iter();
        let wave_tables = wave_tables_tokens(&self.wave_tables);
        let rows = self.rows.iter();
        let frames_per_tick = self.frames_per_tick.to_raw();
        let ticks_per_row = self.ticks_per_row;
        tokens.append_all(quote! {{
            static INSTRUMENTS: &[#i::Instrument] = &[#(#instruments),*];
            static WAVE_TABLES: &[[u8; 16]] = #wave_tables;
            static ROWS: &[#i::PatternSlot] = &[#(#rows),*];
            #i::Sfx {
                channel: #channel,
                instruments: #p::Cow::Borrowed(INSTRUMENTS),
                wave_tables: #p::Cow::Borrowed(WAVE_TABLES),
                rows: #p::Cow::Borrowed(ROWS),
                frames_per_tick: #i::FrameCount::from_raw(#frames_per_tick),
                ticks_per_row: #ticks_per_row,
            }
        }});
    }
}
