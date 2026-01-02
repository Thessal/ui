rsi(rs: Signal<Float>, n: Int) : Signal<Float> = {
    # Relative Strength Index (RSI) : 1-(1/(1+RS))
    rsi_raw : Signal<Float> = divide(dividend=1., divisor=add(x=rs,y=1.))
    result : Signal<Float> = subtract(x=1., y=rsi_raw)
}

stoch(signal: Signal<Float>, n: Int) : Signal<Float> = {
    # 0.2 for oversold, 0.8 for overbought
    signal_min : Signal<Float> = ts_min(signal=signal, period=n)
    signal_max : Signal<Float> = ts_max(signal=signal, period=n)
    result : Signal<Float> = divide(
        dividend=subtract(dividend=signal, divisor=signal_min),
        divisor=subtract(dividend=signal_max, divisor=signal_min))
}

stoch_rsi(signal: Signal<Float>, n: Int) : Signal<Float> = {
    # signal: price, n: RSI window
    # Application of stochastic oscillator to a set of relative strength index values.
    # Measures short-term momentum.

    delta : Signal<Float> = ts_diff(signal=signal, period=1)
    gain_raw : Signal<Float> = max(x=delta, y=0)
    loss_raw : Signal<Float> = abs(signal=min(x=delta, y=0))

    # Relative Strength based on average gain / loss
    avg_gain : Signal<Float> = ts_mean(signal=gain_raw, period=n)
    avg_loss : Signal<Float> = ts_mean(signal=loss_raw, period=n)
    rs : Signal<Float> = divide(dividend=avg_gain, divisor=avg_loss)
    rsi_value : Signal<Float> = rsi(rs=rs, n=n)

    # Stochastic RSI (raw)
    result = stoch(signal=rsi_value, n=n)
}

trigger(signal: Signal<Float>, k: Int, d: Int) : Signal<Float> = {
    # k: %K smoothing window (typically 14), d:%D smoothing window (typically 3)
    # Detects divergences.

    k_smooth : Signal<Float> = ts_mean(signal=stoch_raw, period=k)
    d_smooth : Signal<Float> = ts_mean(signal=k_smooth, period=d)

    upper_trigger : Signal<Float> = greater(signal=k_smooth, thres=0.8)
    lower_trigger : Signal<Float> = less(signal=k_smooth, thres=0.2)
    trigger : Signal<Float> = add(x=upper_trigger, y=lower_trigger)

    # When %k < 0.2 or %k > 0.8, Updates output value to %d
    result : Signal<Float> = tradewhen(signal=d_smooth, enter=trigger, exit=-1., period=252)
} 

stoch_rsi_triggered(signal: Signal<Float>, n: Int) : Signal<Float> = {
    result : Signal<Float> = trigger(signal=stoch_rsi(signal=signal, n=n), k=k, d=d)
}