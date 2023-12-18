
CREATE TABLE IF NOT EXISTS tb_list (
    id          char(64) CONSTRAINT list_id PRIMARY KEY,
    total       int NOT NULL DEFAULT 0 CHECK (total > 0),
    officer     integer NOT NULL DEFAULT 3 CHECK (officer > 0 and officer < 4),
    vault_supply bigint NOT NULL DEFAULT 0,
    vault_balance bigint NOT NULL DEFAULT 0,
    profit_balance bigint NOT NULL DEFAULT 0,
    insurance_balance bigint NOT NULL DEFAULT 0,
    epoch_profit JSON
);
CREATE TABLE IF NOT EXISTS tb_market (
     id          char(64) CONSTRAINT market_id PRIMARY KEY,
     max_leverage integer NOT NULL DEFAULT 0 CHECK (max_leverage > 0),
     insurance_fee bigint NOT NULL DEFAULT 0 CHECK (insurance_fee >= 0),
     margin_fee bigint NOT NULL DEFAULT 0 CHECK (margin_fee >= 0),
     fund_fee bigint NOT NULL DEFAULT 0 CHECK (fund_fee >= 0),
     fund_fee_manual boolean NOT NULL DEFAULT false,
     spread_fee bigint NOT NULL DEFAULT 0 CHECK (spread_fee >= 0),
     spread_fee_manual boolean NOT NULL DEFAULT false,
     status integer NOT NULL DEFAULT 1 CHECK (status > 0 and status < 4),
     long_position_total bigint NOT NULL DEFAULT 0,
     short_position_total bigint NOT NULL DEFAULT 0,
     symbol char(20) NOT NULL DEFAULT '',
     symbol_short char(5) NOT NULL DEFAULT '',
     icon varchar(256) NOT NULL DEFAULT '',
     description varchar(1000) NOT NULL DEFAULT '',
     unit_size bigint NOT NULL DEFAULT 0,
     opening_price bigint NOT NULL DEFAULT 0,
     list_id char(64) NOT NULL DEFAULT ''
);
CREATE INDEX idx_market_status ON tb_market (status);

CREATE TABLE IF NOT EXISTS tb_account (
    id     char(64) CONSTRAINT account_id PRIMARY KEY,
    owner  char(64) NOT NULL,
    offset_idx bigint NOT NULL DEFAULT 0 CHECK (offset_idx >= 0),
    balance bigint NOT NULL DEFAULT 0,
    isolated_balance bigint NOT NULL DEFAULT 0,
    profit bigint NOT NULL DEFAULT 0,
    margin_total bigint NOT NULL DEFAULT 0,
    margin_cross_total bigint NOT NULL DEFAULT 0,
    margin_isolated_total bigint NOT NULL DEFAULT 0,
    margin_cross_buy_total bigint NOT NULL DEFAULT 0,
    margin_cross_sell_total bigint NOT NULL DEFAULT 0,
    margin_isolated_buy_total bigint NOT NULL DEFAULT 0,
    margin_isolated_sell_total bigint NOT NULL DEFAULT 0,
    cross_position_idx JSON,
    isolated_position_idx char(64)[]
);
CREATE INDEX idx_account_owner ON tb_account (owner);

CREATE TABLE IF NOT EXISTS tb_position (
    id     char(64) CONSTRAINT position_id PRIMARY KEY,
    offset_idx bigint NOT NULL DEFAULT 0 CHECK (offset_idx >= 0),
    margin bigint NOT NULL DEFAULT 0,
    margin_balance bigint NOT NULL DEFAULT 0,
    leverage integer NOT NULL DEFAULT 0 CHECK (leverage > 0),
    position_type integer NOT NULL DEFAULT 1 CHECK (position_type > 0 and position_type < 3),
    status integer NOT NULL DEFAULT 1 CHECK (position_type > 0 and position_type < 8),
    direction integer NOT NULL DEFAULT 1 CHECK (position_type > 0 and position_type < 3),
    unit_size bigint NOT NULL DEFAULT 0,
    lot bigint NOT NULL DEFAULT 0,
    open_price bigint NOT NULL DEFAULT 0,
    open_spread bigint NOT NULL DEFAULT 0,
    open_real_price bigint NOT NULL DEFAULT 0,
    close_price bigint NOT NULL DEFAULT 0,
    close_spread bigint NOT NULL DEFAULT 0,
    close_real_price bigint NOT NULL DEFAULT 0,
    profit bigint NOT NULL DEFAULT 0,
    stop_surplus_price bigint NOT NULL DEFAULT 0,
    stop_loss_price bigint NOT NULL DEFAULT 0,
    create_time bigint NOT NULL DEFAULT 0,
    open_time bigint NOT NULL DEFAULT 0,
    close_time bigint NOT NULL DEFAULT 0,
    open_operator char(64) NOT NULL DEFAULT '',
    close_operator char(64) NOT NULL DEFAULT '',
    market_id char(64) NOT NULL DEFAULT '',
    account_id char(64) NOT NULL DEFAULT '',
    symbol char(20) NOT NULL DEFAULT '',
    symbol_short char(5) NOT NULL DEFAULT '',
    icon varchar(256) NOT NULL DEFAULT ''
);

CREATE INDEX idx_position_owner ON tb_position (account_id);
CREATE INDEX idx_position_market ON tb_position (market_id);
CREATE INDEX idx_position_status ON tb_position (status);
CREATE INDEX idx_position_type ON tb_position (position_type);
CREATE INDEX idx_position_direction ON tb_position (direction);