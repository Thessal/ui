# Money‑Flow Multiplier
mfm(high : Signal<Float>, low : Signal<Float>, close : Signal<Float>) : Signal<Float> = {
    # close price ralateve to high-low range (so called "hammer" pattern, or doji Candlestick)
    # ((close‑low) – (high‑close)) / (high‑low)
    numerator : Signal<Float> = subtract(x=range(upper=close, lower=low), y=range(upper=high, lower=close))
    price_range : Signal<Float> = range(upper=high, lower=low)
    result : Signal<Float> = divide(dividend=numerator, divisor=price_range)
}

# Chaikin Money Flow
# Volume weighted average signed volume. Similar to ADL.
cmf(
    high   : Signal<Float>,
    low    : Signal<Float>,
    close  : Signal<Float>,
    volume : Signal<Float>,
    period : Int  
) : Signal<Float> = {
    # Money‑Flow Multiplier (MFM)
    mfm      : Signal<Float> = mfm(high=high, low=low, close=close)

    # Money‑Flow Volume (MFV)
    mfv      : Signal<Float> = multiply(x=mfm, y=volume)

    # Chaikin Money Flow = Σ(MFV) / Σ(volume)
    mfv_mean : Signal<Float> = ts_mean(signal=mfv, period=period)
    vol_mean : Signal<Float> = ts_mean(signal=volume, period=period)
    result   : Signal<Float> = divide(dividend=mfv_mean, divisor=vol_mean)
}

# Accumulation/Distribution Line (ADL)
adl(high : Signal<Float>, low : Signal<Float>, close : Signal<Float>, volume : Signal<Float>, period : Int) : Signal<Float> = {
    # Volume weighted averaged signed volume, that measures underlying supply and demand.
    money_flow_multiplier : Signal<Float> = mfm(high=high, low=low, close=close)

    # Money‑Flow Volume = Money‑Flow Multiplier * volume
    money_flow_volume : Signal<Float> = multiply(x=money_flow_multiplier, y=volume)

    # ADL = cumulative sum of Money‑Flow Volume.
    result = ts_decay_linear(signal=money_flow_volume, period=period)
}

# Chaikin Oscillator
chaikin_oscillator(
    high   : Signal<Float>,
    low    : Signal<Float>,
    close  : Signal<Float>,
    volume : Signal<Float>,
    period : Int,
    period1 : Int, 
    period2 : Int
) : Signal<Float> = {
    # Typical value of period1 = 3, period2 = 10
    adl = adl(high=high, low=low, close=cloe, volume=volume, period=period)

    ema_1  : Signal<Float> = ts_mean_exponential(signal= adl, period=period1)
    ema_2 : Signal<Float> = ts_mean_exponential(signal= adl, period=period2)
    result : Signal<Float> = subtract(x=ema_1, y=ema_2)
}

# Chaikin Volatility 
chaikin_volatility(high: Signal<Float>, low: Signal<Float>, period1 : Int, period2 : Int) : Signal<Float> = {
    price_range : Signal<Float> = subtract(x = signal_high, y = signal_low)
    sma_1 : Signal<Float> = ts_mean(signal = price_range, period = period1)
    sma_2 : Signal<Float> = ts_mean(signal = price_range, period = period2)
    result : Signal<Float> = subtract(x = sma_1, y = sma_2)
}