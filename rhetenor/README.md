# Rhetenor and Didius

## Rhetenor

### Functions
* Performance analysis
* Selection
* Combination

### Quality Benchmark (without RAG)
Task : Technical indicator generation using [Morpho](https://github.com/Thessal/transpiler), DOW 30 stocks, 1-minute OHLCV  

* Alpha quality by information type

| data_type | parse pct | validation pct | ann returns pct | ann sharpe | mdd pct | min coverage | max position | prompt length | total count | success count |
|----------:|----------:|---------------:|----------------:|-----------:|--------:|-------------:|-------------:|--------------:|------------:|--------------:|
|  document |     27.57 |           0.54 |            0.19 |       0.55 |   -0.07 |         1.00 |         0.06 |      1,524.88 |        4276 |            23 |
|   summary |     33.46 |           0.59 |            0.18 |       0.38 |   -0.08 |         1.00 |         0.07 |        699.56 |        5391 |            32 |

* Alpha quality by agent type 

|          agent | parse pct | validation pct | ann returns pct | ann sharpe | mdd pct | min coverage | max position | prompt length | total count | success count |
|---------------:|----------:|---------------:|----------------:|-----------:|--------:|-------------:|-------------:|--------------:|------------:|--------------:|
|   human#000001 |     19.24 |           0.38 |            0.47 |       0.73 |   -0.05 |         1.00 |         0.05 |      1,540.29 |         790 |             3 |
|   human#000002 |     29.46 |           0.57 |            0.15 |       0.52 |   -0.07 |         1.00 |         0.07 |      1,521.39 |        3486 |            20 |
| machine#FFFFFF |     33.46 |           0.59 |            0.18 |       0.38 |   -0.08 |         1.00 |         0.07 |        699.56 |        5391 |            32 |


* Alpha quality by prompt length

| prompt length | parse pct | validation pct | ann returns pct | ann sharpe | mdd pct | min coverage | max position | prompt length | total count | success count |
|--------------:|----------:|---------------:|----------------:|-----------:|--------:|-------------:|-------------:|--------------:|------------:|--------------:|
|             0 |     40.75 |           0.37 |            0.23 |       0.36 |   -0.07 |         1.00 |         0.06 |        283.63 |        2736 |            10 |
|           500 |     29.73 |           0.89 |            0.14 |       0.45 |   -0.07 |         1.00 |         0.07 |        735.24 |        2593 |            23 |
|          1000 |     27.84 |           0.32 |            0.14 |       0.28 |   -0.08 |         1.00 |         0.07 |      1,250.71 |        1857 |             6 |
|          1500 |     27.64 |           0.88 |            0.19 |       0.66 |   -0.07 |         1.00 |         0.07 |      1,736.45 |        1136 |            10 |
|          2000 |     18.01 |           0.35 |            0.51 |       0.79 |   -0.10 |         1.00 |         0.09 |      2,210.02 |         855 |             3 |
|          2500 |     25.53 |           0.61 |            0.05 |       0.05 |   -0.10 |         1.00 |         0.10 |      2,734.88 |         329 |             2 |
|          3000 |     17.88 |           0.66 |            0.07 |       0.16 |   -0.07 |         1.00 |         0.10 |      3,193.10 |         151 |             1 |
|          4000 |     10.00 |           0.00 |             NaN |        NaN |   -2.72 |         1.00 |         1.00 |      4,256.00 |          10 |             0 |


* Process Management
  * Base production rate = 0.2/s
  * Parse success rate = 0.062/s
  * Test pass rate = 0.0011/s

## Didius 
* Execution