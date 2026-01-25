
# KOSPI Master File Spec
URL = "https://new.real.download.dws.co.kr/common/master/kospi_code.mst.zip"
FILENAME = "kospi_code.mst"
PARSER_TYPE = "fixed_suffix"
SUFFIX_LENGTH = 228

FIELD_SPECS = [
    2, 1, 4, 4, 4,              # group_code .. industry_small
    1, 1, 1, 1, 1,              # manufacturing .. kospi100
    1, 1, 1, 1, 1,              # kospi50 .. krx100
    1, 1, 1, 1, 1,              # krx_auto .. spac
    1, 1, 1, 1, 1,              # krx_energy_chemical .. krx_construction
    1, 1, 1, 1, 1,              # non1 .. krx_sector_transport
    1, 9, 5, 5, 1,              # sri .. trading_halt
    1, 1, 2, 1, 1,              # liquidation .. dishonest_disclosure
    1, 2, 2, 2, 3,              # bypass_listing .. margin_rate
    1, 3, 12, 12, 8,            # credit_available .. listing_date
    15, 21, 2, 7, 1,            # listed_shares .. preferred_stock
    1, 1, 1, 1, 9,              # short_sale_overheat .. sales
    9, 9, 5, 9, 8,              # operating_profit .. base_year_month
    9, 3, 1, 1, 1               # market_cap .. securities_lending_available
]

COLUMNS = [
    'group_code', 'market_cap_scale', 'industry_large', 'industry_medium', 'industry_small',
    'manufacturing', 'low_liquidity', 'governance_index_stock', 'kospi200_sector_industry', 'kospi100',
    'kospi50', 'krx', 'etp', 'elw_issuance', 'krx100',
    'krx_auto', 'krx_semiconductor', 'krx_bio', 'krx_bank', 'spac',
    'krx_energy_chemical', 'krx_steel', 'short_term_overheat', 'krx_media_telecom', 'krx_construction',
    'non1', 'krx_security', 'krx_ship', 'krx_sector_insurance', 'krx_sector_transport',
    'sri', 'base_price', 'trading_unit', 'after_hours_unit', 'trading_halt',
    'liquidation', 'management_stock', 'market_warning', 'warning_forecast', 'dishonest_disclosure',
    'bypass_listing', 'lock_division', 'par_value_change', 'capital_increase', 'margin_rate',
    'credit_available', 'credit_period', 'prev_day_volume', 'par_value', 'listing_date',
    'listed_shares', 'capital', 'settlement_month', 'public_offering_price', 'preferred_stock',
    'short_sale_overheat', 'unusual_rise', 'krx300', 'kospi', 'sales',
    'operating_profit', 'ordinary_profit', 'net_income', 'roe', 'base_year_month',
    'market_cap', 'group_company_code', 'company_credit_limit_exceed', 'collateral_loan_available', 'securities_lending_available'
]
