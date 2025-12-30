"""
Chain and ChainAsset Admin CRUD
SOP Phase 2: Chain Config Tab
"""
from fastapi_amis_admin.admin import admin
from models import Chain, ChainAsset


class ChainAdmin(admin.ModelAdmin):
    """Admin interface for Blockchain Chain management"""
    
    page_schema = admin.PageSchema(label="Chains", icon="fa fa-link")
    pk_name = "chain_slug"
    model = Chain
    
    list_display = [
        Chain.chain_slug,
        Chain.chain_name,
        Chain.network_id,
        Chain.confirmation_blocks,
        Chain.is_active,
        Chain.created_at,
    ]
    
    ordering = [Chain.chain_slug]
    search_fields = [Chain.chain_slug, Chain.chain_name]
    enable_bulk_create = False


class ChainAssetAdmin(admin.ModelAdmin):
    """Admin interface for Chain Asset bindings (ADR-005)"""
    
    page_schema = admin.PageSchema(label="Chain Assets", icon="fa fa-cube")
    pk_name = "id"
    model = ChainAsset
    
    list_display = [
        ChainAsset.id,
        ChainAsset.chain_slug,
        ChainAsset.asset_id,
        ChainAsset.contract_address,
        ChainAsset.decimals,
        ChainAsset.min_deposit,
        ChainAsset.withdraw_fee,
        ChainAsset.is_active,
        ChainAsset.created_at,
    ]
    
    ordering = [ChainAsset.id.desc()]
    search_fields = [ChainAsset.contract_address]
    enable_bulk_create = False
    
    # SECURITY: is_active defaults to False in model
    # All fields updatable except id
    update_fields = [
        ChainAsset.contract_address,
        ChainAsset.decimals,
        ChainAsset.min_deposit,
        ChainAsset.min_withdraw,
        ChainAsset.withdraw_fee,
        ChainAsset.is_active,
    ]
