"""
SQLAlchemy Models for Admin Dashboard
Matches existing PostgreSQL schema from migrations/
"""

from datetime import datetime
from decimal import Decimal
from typing import Optional

from sqlalchemy import (
    BigInteger,
    Boolean,
    Column,
    DateTime,
    Integer,
    Numeric,
    SmallInteger,
    String,
    Text,
    ForeignKey,
    CheckConstraint,
    Index,
    func,
)
from sqlalchemy.dialects.postgresql import JSONB
from sqlalchemy.ext.asyncio import AsyncAttrs
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship


class Base(AsyncAttrs, DeclarativeBase):
    """Base class for all models"""
    pass



class Asset(Base):
    """
    Asset model - matches assets_tb
    From: migrations/001_init_schema.sql
    """
    __tablename__ = "assets_tb"
    
    asset_id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    asset: Mapped[str] = mapped_column(String(16), unique=True, nullable=False)
    name: Mapped[str] = mapped_column(String(64), nullable=False)
    decimals: Mapped[int] = mapped_column(SmallInteger, nullable=False)
    status: Mapped[int] = mapped_column(SmallInteger, nullable=False, default=1)  # 0=disabled, 1=active
    asset_flags: Mapped[int] = mapped_column(Integer, nullable=False, default=7)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), default=func.now())
    
    __table_args__ = (
        CheckConstraint("asset = UPPER(asset)", name="chk_asset_uppercase"),
    )


class Symbol(Base):
    """
    Symbol model - matches symbols_tb
    From: migrations/001_init_schema.sql + 006_trade_fee.sql
    """
    __tablename__ = "symbols_tb"
    
    symbol_id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    symbol: Mapped[str] = mapped_column(String(32), unique=True, nullable=False)
    base_asset_id: Mapped[int] = mapped_column(Integer, ForeignKey("assets_tb.asset_id"), nullable=False)
    quote_asset_id: Mapped[int] = mapped_column(Integer, ForeignKey("assets_tb.asset_id"), nullable=False)
    price_decimals: Mapped[int] = mapped_column(SmallInteger, nullable=False)
    qty_decimals: Mapped[int] = mapped_column(SmallInteger, nullable=False)
    min_qty: Mapped[int] = mapped_column(BigInteger, nullable=False, default=0)
    status: Mapped[int] = mapped_column(SmallInteger, nullable=False, default=1)  # 0=offline, 1=online, 2=maintenance
    symbol_flags: Mapped[int] = mapped_column(Integer, nullable=False, default=15)
    base_maker_fee: Mapped[int] = mapped_column(Integer, nullable=False, default=1000)  # 0.10%
    base_taker_fee: Mapped[int] = mapped_column(Integer, nullable=False, default=2000)  # 0.20%
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), default=func.now())
    
    __table_args__ = (
        CheckConstraint("symbol = UPPER(symbol)", name="chk_symbol_uppercase"),
    )


class VIPLevel(Base):
    """
    VIP Level model - matches vip_levels_tb
    From: migrations/006_trade_fee.sql
    """
    __tablename__ = "vip_levels_tb"
    
    level: Mapped[int] = mapped_column(SmallInteger, primary_key=True)
    discount_percent: Mapped[int] = mapped_column(SmallInteger, nullable=False, default=100)  # 100 = no discount
    min_volume: Mapped[Optional[int]] = mapped_column(BigInteger, default=0)
    description: Mapped[Optional[str]] = mapped_column(String(64))


class AdminAuditLog(Base):
    """
    Admin Audit Log - matches admin_audit_log
    From: migrations/007_admin_audit_log.sql
    UX-10: trace_id for evidence chain
    """
    __tablename__ = "admin_audit_log"
    
    id: Mapped[int] = mapped_column(BigInteger, primary_key=True, autoincrement=True)
    trace_id: Mapped[Optional[str]] = mapped_column(String(26))  # UX-10: ULID format
    admin_id: Mapped[int] = mapped_column(BigInteger, nullable=False)
    admin_username: Mapped[Optional[str]] = mapped_column(String(64))
    ip_address: Mapped[str] = mapped_column(String(45), nullable=False)  # IPv6 support
    action: Mapped[str] = mapped_column(String(32), nullable=False)  # GET/POST/PUT/DELETE
    path: Mapped[str] = mapped_column(String(256), nullable=False)
    entity_type: Mapped[Optional[str]] = mapped_column(String(32))  # asset/symbol/vip_level
    entity_id: Mapped[Optional[int]] = mapped_column(BigInteger)
    old_value: Mapped[Optional[dict]] = mapped_column(JSONB)
    new_value: Mapped[Optional[dict]] = mapped_column(JSONB)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), default=func.now())

    
    __table_args__ = (
        Index("idx_audit_admin_id", "admin_id"),
        Index("idx_audit_created_at", "created_at"),
        Index("idx_audit_entity", "entity_type", "entity_id"),
        Index("idx_audit_trace_id", "trace_id"),  # UX-10
    )


class Chain(Base):
    """
    Chain model - matches chains_tb
    From: migrations/012_chain_assets.sql (ADR-005)
    """
    __tablename__ = "chains_tb"
    
    chain_slug: Mapped[str] = mapped_column(String(32), primary_key=True)
    chain_name: Mapped[str] = mapped_column(String(64), nullable=False)
    network_id: Mapped[Optional[str]] = mapped_column(String(32))
    scan_start_height: Mapped[int] = mapped_column(BigInteger, nullable=False, default=0)
    confirmation_blocks: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    is_active: Mapped[bool] = mapped_column(Boolean, default=True)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), default=func.now())
    updated_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), default=func.now(), onupdate=func.now())


class ChainAsset(Base):
    """
    Chain Asset model - matches chain_assets_tb
    From: migrations/012_chain_assets.sql (ADR-005)
    Physical binding of logical assets to blockchain contracts
    """
    __tablename__ = "chain_assets_tb"
    
    id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    chain_slug: Mapped[str] = mapped_column(String(32), ForeignKey("chains_tb.chain_slug"), nullable=False)
    asset_id: Mapped[int] = mapped_column(Integer, ForeignKey("assets_tb.asset_id"), nullable=False)
    contract_address: Mapped[Optional[str]] = mapped_column(String(128))  # NULL for native assets
    decimals: Mapped[int] = mapped_column(SmallInteger, nullable=False)
    min_deposit: Mapped[Optional[int]] = mapped_column(BigInteger, default=0)   # Atomic units (Satoshis/Wei)
    min_withdraw: Mapped[Optional[int]] = mapped_column(BigInteger, default=0)  # Atomic units
    withdraw_fee: Mapped[Optional[int]] = mapped_column(BigInteger, default=0)  # Atomic units
    is_active: Mapped[bool] = mapped_column(Boolean, default=False)  # SECURITY: Default inactive
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), default=func.now())
    updated_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), default=func.now(), onupdate=func.now())
