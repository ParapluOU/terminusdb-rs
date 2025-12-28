# Python code to extract XSD schema information
# This is embedded in the Rust binary and executed via PyO3

import json
from pathlib import Path

def create_uri_mapper(catalog_path):
    """Create a URI mapper function for resolving URNs to local file paths.

    Uses a simple text-based catalog parser for DITA/OASIS catalog files.
    """
    from pathlib import Path as PathLib
    import sys

    urn_map = {}
    current_base = None
    catalog_dir = PathLib(catalog_path).parent

    # Debug logging disabled for cleaner output
    # print(f"[CATALOG] Parsing catalog: {catalog_path}", file=sys.stderr)
    # print(f"[CATALOG] Catalog directory: {catalog_dir}", file=sys.stderr)

    with open(catalog_path, 'r') as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('--'):
                continue

            if line.startswith('BASE'):
                parts = line.split('"')
                if len(parts) >= 2:
                    current_base = parts[1]
                    # Strip leading ../../ from BASE paths (catalog designed for different layout)
                    if current_base.startswith('../../'):
                        current_base = current_base[6:]  # Remove '../../'
                        # print(f"[CATALOG] Set BASE: {parts[1]} (stripped to: {current_base})", file=sys.stderr)
                    # else:
                        # print(f"[CATALOG] Set BASE: {current_base}", file=sys.stderr)

            elif line.startswith('URI'):
                parts = line.split('"')
                if len(parts) >= 4:
                    urn = parts[1]
                    local_path = parts[3]
                    if current_base:
                        full_path = catalog_dir / current_base / local_path
                    else:
                        full_path = catalog_dir / local_path
                    urn_map[urn] = str(full_path.resolve())

    # print(f"[CATALOG] Loaded {len(urn_map)} URN mappings", file=sys.stderr)
    # if len(urn_map) > 0:
        # print(f"[CATALOG] Sample mappings (first 5):", file=sys.stderr)
        # for i, (urn, path) in enumerate(list(urn_map.items())[:5]):
            # print(f"[CATALOG]   {urn} -> {path}", file=sys.stderr)

    def uri_mapper(uri):
        from urllib.parse import unquote
        # import sys

        # xmlschema may URL-encode the URN, so try both encoded and decoded
        # print(f"[URI_MAPPER] Input URI: {repr(uri)}", file=sys.stderr)

        if uri is None:
            # print(f"[URI_MAPPER] WARNING: Received None as URI!", file=sys.stderr)
            return None

        decoded_uri = unquote(uri)
        # print(f"[URI_MAPPER] Decoded URI: {repr(decoded_uri)}", file=sys.stderr)

        resolved = urn_map.get(decoded_uri) or urn_map.get(uri)

        if resolved:
            # print(f"[URI_MAPPER] ✓ Resolved to: {resolved}", file=sys.stderr)
            return resolved
        else:
            # print(f"[URI_MAPPER] ✗ Could not resolve, returning original URI unchanged", file=sys.stderr)
            # Return the original URI unchanged - it may be a regular file path, not a URN
            return uri

    return uri_mapper

def extract_element_info(element):
    """Extract detailed info about an element."""
    info = {
        'name': element.qualified_name or element.name,
        'qualified_name': element.qualified_name or element.name,
        'type': extract_type_info(element.type) if element.type else None,
        'min_occurs': element.min_occurs if hasattr(element, 'min_occurs') else 1,
        'max_occurs': element.max_occurs if hasattr(element, 'max_occurs') else 1,
        'nillable': element.nillable if hasattr(element, 'nillable') else False,
        'default': element.default if hasattr(element, 'default') else None,
    }
    return info

def extract_type_info(type_obj):
    """Extract detailed info about a type."""
    info = {
        'name': type_obj.qualified_name if hasattr(type_obj, 'qualified_name') else None,
        'qualified_name': type_obj.qualified_name if hasattr(type_obj, 'qualified_name') else None,
        'category': type(type_obj).__name__,
        'is_complex': 'Complex' in type(type_obj).__name__,
        'is_simple': 'Simple' in type(type_obj).__name__,
    }

    # For complex types, get content model
    if hasattr(type_obj, 'content') and type_obj.content:
        info['content_model'] = type(type_obj.content).__name__

    # Get attributes
    if hasattr(type_obj, 'attributes') and type_obj.attributes:
        info['attributes'] = [
            {
                'name': attr.name or 'unknown',
                'type': (attr.type.qualified_name if hasattr(attr.type, 'qualified_name') and attr.type.qualified_name else str(attr.type)) if attr.type else 'xs:string',
                'use': attr.use if hasattr(attr, 'use') and attr.use else 'optional',
                'default': attr.default if hasattr(attr, 'default') else None,
            }
            for attr in type_obj.attributes.values()
        ]

    # Get elements for complex types
    if hasattr(type_obj, 'content') and type_obj.content and hasattr(type_obj.content, 'iter_elements'):
        try:
            elements = list(type_obj.content.iter_elements())
            if elements:
                info['child_elements'] = [
                    {
                        'name': elem.qualified_name or elem.name or 'unknown',
                        'type': (elem.type.qualified_name if hasattr(elem.type, 'qualified_name') and elem.type.qualified_name else type(elem.type).__name__) if elem.type else 'unknown',
                        'min_occurs': elem.min_occurs if hasattr(elem, 'min_occurs') else 1,
                        'max_occurs': elem.max_occurs if hasattr(elem, 'max_occurs') else 1,
                    }
                    for elem in elements
                ]
        except:
            pass  # Ignore errors iterating elements

    return info

def extract_complex_type_info(type_obj):
    """Extract info for complex type definition."""
    info = {
        'name': type_obj.qualified_name or type_obj.name,
        'qualified_name': type_obj.qualified_name or type_obj.name,
        'category': type(type_obj).__name__,
        'is_complex': True,
        'is_simple': False,
    }

    # Content model
    if hasattr(type_obj, 'content') and type_obj.content:
        info['content_model'] = type(type_obj.content).__name__

    # Attributes
    if hasattr(type_obj, 'attributes') and type_obj.attributes:
        info['attributes'] = [
            {
                'name': attr.name or 'unknown',
                'type': (attr.type.qualified_name if hasattr(attr.type, 'qualified_name') and attr.type.qualified_name else str(attr.type)) if attr.type else 'xs:string',
                'use': attr.use if hasattr(attr, 'use') and attr.use else 'optional',
                'default': attr.default if hasattr(attr, 'default') else None,
            }
            for attr in type_obj.attributes.values()
        ]

    # Child elements
    if hasattr(type_obj, 'content') and type_obj.content and hasattr(type_obj.content, 'iter_elements'):
        try:
            elements = list(type_obj.content.iter_elements())
            if elements:
                info['child_elements'] = [
                    {
                        'name': elem.qualified_name or elem.name or 'unknown',
                        'type': (elem.type.qualified_name if hasattr(elem.type, 'qualified_name') and elem.type.qualified_name else type(elem.type).__name__) if elem.type else 'unknown',
                        'min_occurs': elem.min_occurs if hasattr(elem, 'min_occurs') else 1,
                        'max_occurs': elem.max_occurs if hasattr(elem, 'max_occurs') else 1,
                    }
                    for elem in elements
                ]
        except:
            pass

    return info

def extract_simple_type_info(type_obj):
    """Extract info for simple type definition."""
    info = {
        'name': type_obj.qualified_name or type_obj.name,
        'qualified_name': type_obj.qualified_name or type_obj.name,
        'category': type(type_obj).__name__,
        'base_type': type_obj.base_type.qualified_name if hasattr(type_obj, 'base_type') and type_obj.base_type else None,
        'restrictions': None,
    }
    return info

# Note: Main extraction code is now handled in Rust (schema_model.rs)
# This file only provides helper functions for extraction
