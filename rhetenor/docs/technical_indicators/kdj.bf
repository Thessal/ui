# KDJ j line
kdj_wrapper(signal_high: Signal<Float>,
            signal_low: Signal<Float>,
            signal_close: Signal<Float>,
            lookback: Int,
            smooth: Int) : Signal<Float> = {
    # typical parameter: lookback, value = 14, 3 
    lh_min : Signal<Float> = ts_min(signal=signal_low,  period=lookback)
    hh_max : Signal<Float> = ts_max(signal=signal_high, period=lookback)

    k_line : Signal<Float> = divide(
                dividend = subtract(signal=signal_close, lh_min),
                divisor  = subtract(signal=hh_max, lh_min))

    d_line : Signal<Float> = ts_mean(signal=k_line, period=smooth)

    three_k : Signal<Float> = multiply(signal=k_line, 3.)
    two_d   : Signal<Float> = multiply(signal=d_line, 2.)

    j_line : Signal<Float> = subtract(signal=three_k, two_d)

    result : Signal<Float> = j_line
}