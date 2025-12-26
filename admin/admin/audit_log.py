"""
Audit Log Admin (Read-Only)
AC-13: All CRUD ops queryable
"""

from fastapi_amis_admin.admin import admin

from models import AdminAuditLog


class AuditLogAdmin(admin.ModelAdmin):
    """Admin interface for Audit Log - READ ONLY"""
    
    page_schema = admin.PageSchema(label="Audit Log", icon="fa fa-history")
    pk_name = "id"  # Specify primary key name
    model = AdminAuditLog
    
    # List columns
    list_display = [
        AdminAuditLog.id,
        AdminAuditLog.admin_username,
        AdminAuditLog.ip_address,
        AdminAuditLog.action,
        AdminAuditLog.path,
        AdminAuditLog.entity_type,
        AdminAuditLog.entity_id,
        AdminAuditLog.created_at,
    ]
    
    # Search and filter
    search_fields = [AdminAuditLog.admin_username, AdminAuditLog.path, AdminAuditLog.entity_type]
    
    # READ ONLY - disable all modifications
    enable_bulk_create = False
    enable_bulk_delete = False  # Explicitly disable bulk delete
    readonly = True  # Mark as read-only for audit purposes
    
    # Disable create/update/delete operations
    async def has_create_permission(self, request, data=None, **kwargs) -> bool:
        return False
    
    async def has_update_permission(self, request, item_id=None, data=None, **kwargs) -> bool:
        return False
    
    async def has_delete_permission(self, request, item_id=None, **kwargs) -> bool:
        return False
