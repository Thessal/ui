# Stochastic %K (smoothed)
stoch_k(
    high   : Signal<Float>,
    low    : Signal<Float>,
    close  : Signal<Float>,
    window : Int   = 14,
    smooth_k : Int = 3
) : Signal<Float> = {
    # 1. Highest high and lowest low over the look‑back window
    hh : Signal<Float> = ts_max(signal = high, period = window)
    ll : Signal<Float> = ts_min(signal = low , period = window)

    # 2. Raw %K = 100 * (close – ll) / (hh – ll)
    diff_high_low : Signal<Float> = subtract(dividend = hh, divisor = ll)   # hh - ll
    diff_close_ll : Signal<Float> = subtract(dividend = close, divisor = ll) # close - ll
    raw_k : Signal<Float> = multiply(
        x = const(value = 100.),
        y = divide(dividend = diff_close_ll, divisor = diff_high_low)
    )

    # 3. Smoothed %K = SMA(raw_k, smooth_k)
    result : Signal<Float> = ts_mean(signal = raw_k, period = smooth_k)
}

# Stochastic %D (moving‑average of the smoothed %K)
stoch_d(
    high    : Signal<Float>,
    low     : Signal<Float>,
    close   : Signal<Float>,
    window  : Int   = 14,
    smooth_k : Int = 3,
    smooth_d : Int = 3
) : Signal<Float> = {
    # Re‑use the %K routine – this creates an internal dependency graph.
    k_smoothed : Signal<Float> = stoch_k(
        high = high,
        low  = low,
        close = close,
        window = window,
        smooth_k = smooth_k
    )

    # %D is just an SMA of the already‑smoothed %K
    result : Signal<Float> = ts_mean(signal = k_smoothed, period = smooth_d)
}