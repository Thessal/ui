range(upper : Signal<Float>, lower : Signal<Float>) : Signal<Float> = {
    # Calcuates range from two boundary. upper - lower
    epsilon : Signal<Float> = const(value=1e-10)
    result : Signal<Float> = add(x=subtract(x=upper, y=lower), y=eps)
}

# Money‑Flow Multiplier
mfm(high : Signal<Float>, low : Signal<Float>, close : Signal<Float>) : Signal<Float> = {
    # ((close‑low) – (high‑close)) / (high‑low)
    numerator : Signal<Float> = subtract(x=range(upper=close, lower=low), y=range(upper=high, lower=close))
    price_range : Signal<Float> = range(upper=high, lower=low)
    result : Signal<Float> = divide(dividend=numerator, divisor=price_range)
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