"""Helper functions for working with Angzarr proto types."""

from datetime import datetime
from typing import Optional, Union
from uuid import UUID as PyUUID

from google.protobuf.timestamp_pb2 import Timestamp
from google.protobuf.any_pb2 import Any as ProtoAny

from .proto.angzarr import (
    UUID,
    Cover,
    Edition,
    DomainDivergence,
    EventBook,
    CommandBook,
    Query,
    EventPage,
    CommandPage,
    SequenceRange,
    TemporalQuery,
)
from .errors import InvalidTimestampError

# Constants matching Rust proto_ext::constants
UNKNOWN_DOMAIN = "unknown"
WILDCARD_DOMAIN = "*"
DEFAULT_EDITION = "angzarr"
META_ANGZARR_DOMAIN = "_angzarr"
PROJECTION_DOMAIN_PREFIX = "projection:"
CORRELATION_ID_HEADER = "x-correlation-id"
TYPE_URL_PREFIX = "type.googleapis.com/"


# Type for Cover-bearing objects
CoverBearer = Union[EventBook, CommandBook, Query, Cover]
from inspect import signature as _mutmut_signature
from typing import Annotated
from typing import Callable
from typing import ClassVar


MutantDict = Annotated[dict[str, Callable], "Mutant"]


def _mutmut_trampoline(orig, mutants, call_args, call_kwargs, self_arg = None):
    """Forward call to original or mutated function, depending on the environment"""
    import os
    mutant_under_test = os.environ['MUTANT_UNDER_TEST']
    if mutant_under_test == 'fail':
        from mutmut.__main__ import MutmutProgrammaticFailException
        raise MutmutProgrammaticFailException('Failed programmatically')      
    elif mutant_under_test == 'stats':
        from mutmut.__main__ import record_trampoline_hit
        record_trampoline_hit(orig.__module__ + '.' + orig.__name__)
        result = orig(*call_args, **call_kwargs)
        return result
    prefix = orig.__module__ + '.' + orig.__name__ + '__mutmut_'
    if not mutant_under_test.startswith(prefix):
        result = orig(*call_args, **call_kwargs)
        return result
    mutant_name = mutant_under_test.rpartition('.')[-1]
    if self_arg is not None:
        # call to a class method where self is not bound
        result = mutants[mutant_name](self_arg, *call_args, **call_kwargs)
    else:
        result = mutants[mutant_name](*call_args, **call_kwargs)
    return result


def x_cover_of__mutmut_orig(obj: CoverBearer) -> Optional[Cover]:
    """Extract the Cover from various proto types."""
    if isinstance(obj, Cover):
        return obj
    if hasattr(obj, "cover"):
        return obj.cover
    return None


def x_cover_of__mutmut_1(obj: CoverBearer) -> Optional[Cover]:
    """Extract the Cover from various proto types."""
    if isinstance(obj, Cover):
        return obj
    if hasattr(None, "cover"):
        return obj.cover
    return None


def x_cover_of__mutmut_2(obj: CoverBearer) -> Optional[Cover]:
    """Extract the Cover from various proto types."""
    if isinstance(obj, Cover):
        return obj
    if hasattr(obj, None):
        return obj.cover
    return None


def x_cover_of__mutmut_3(obj: CoverBearer) -> Optional[Cover]:
    """Extract the Cover from various proto types."""
    if isinstance(obj, Cover):
        return obj
    if hasattr("cover"):
        return obj.cover
    return None


def x_cover_of__mutmut_4(obj: CoverBearer) -> Optional[Cover]:
    """Extract the Cover from various proto types."""
    if isinstance(obj, Cover):
        return obj
    if hasattr(obj, ):
        return obj.cover
    return None


def x_cover_of__mutmut_5(obj: CoverBearer) -> Optional[Cover]:
    """Extract the Cover from various proto types."""
    if isinstance(obj, Cover):
        return obj
    if hasattr(obj, "XXcoverXX"):
        return obj.cover
    return None


def x_cover_of__mutmut_6(obj: CoverBearer) -> Optional[Cover]:
    """Extract the Cover from various proto types."""
    if isinstance(obj, Cover):
        return obj
    if hasattr(obj, "COVER"):
        return obj.cover
    return None

x_cover_of__mutmut_mutants : ClassVar[MutantDict] = {
'x_cover_of__mutmut_1': x_cover_of__mutmut_1, 
    'x_cover_of__mutmut_2': x_cover_of__mutmut_2, 
    'x_cover_of__mutmut_3': x_cover_of__mutmut_3, 
    'x_cover_of__mutmut_4': x_cover_of__mutmut_4, 
    'x_cover_of__mutmut_5': x_cover_of__mutmut_5, 
    'x_cover_of__mutmut_6': x_cover_of__mutmut_6
}

def cover_of(*args, **kwargs):
    result = _mutmut_trampoline(x_cover_of__mutmut_orig, x_cover_of__mutmut_mutants, args, kwargs)
    return result 

cover_of.__signature__ = _mutmut_signature(x_cover_of__mutmut_orig)
x_cover_of__mutmut_orig.__name__ = 'x_cover_of'


def x_domain__mutmut_orig(obj: CoverBearer) -> str:
    """Get the domain from a Cover-bearing type, or UNKNOWN_DOMAIN if missing."""
    c = cover_of(obj)
    if c is None or not c.domain:
        return UNKNOWN_DOMAIN
    return c.domain


def x_domain__mutmut_1(obj: CoverBearer) -> str:
    """Get the domain from a Cover-bearing type, or UNKNOWN_DOMAIN if missing."""
    c = None
    if c is None or not c.domain:
        return UNKNOWN_DOMAIN
    return c.domain


def x_domain__mutmut_2(obj: CoverBearer) -> str:
    """Get the domain from a Cover-bearing type, or UNKNOWN_DOMAIN if missing."""
    c = cover_of(None)
    if c is None or not c.domain:
        return UNKNOWN_DOMAIN
    return c.domain


def x_domain__mutmut_3(obj: CoverBearer) -> str:
    """Get the domain from a Cover-bearing type, or UNKNOWN_DOMAIN if missing."""
    c = cover_of(obj)
    if c is None and not c.domain:
        return UNKNOWN_DOMAIN
    return c.domain


def x_domain__mutmut_4(obj: CoverBearer) -> str:
    """Get the domain from a Cover-bearing type, or UNKNOWN_DOMAIN if missing."""
    c = cover_of(obj)
    if c is not None or not c.domain:
        return UNKNOWN_DOMAIN
    return c.domain


def x_domain__mutmut_5(obj: CoverBearer) -> str:
    """Get the domain from a Cover-bearing type, or UNKNOWN_DOMAIN if missing."""
    c = cover_of(obj)
    if c is None or c.domain:
        return UNKNOWN_DOMAIN
    return c.domain

x_domain__mutmut_mutants : ClassVar[MutantDict] = {
'x_domain__mutmut_1': x_domain__mutmut_1, 
    'x_domain__mutmut_2': x_domain__mutmut_2, 
    'x_domain__mutmut_3': x_domain__mutmut_3, 
    'x_domain__mutmut_4': x_domain__mutmut_4, 
    'x_domain__mutmut_5': x_domain__mutmut_5
}

def domain(*args, **kwargs):
    result = _mutmut_trampoline(x_domain__mutmut_orig, x_domain__mutmut_mutants, args, kwargs)
    return result 

domain.__signature__ = _mutmut_signature(x_domain__mutmut_orig)
x_domain__mutmut_orig.__name__ = 'x_domain'


def x_correlation_id__mutmut_orig(obj: CoverBearer) -> str:
    """Get the correlation_id from a Cover-bearing type, or empty string if missing."""
    c = cover_of(obj)
    if c is None:
        return ""
    return c.correlation_id


def x_correlation_id__mutmut_1(obj: CoverBearer) -> str:
    """Get the correlation_id from a Cover-bearing type, or empty string if missing."""
    c = None
    if c is None:
        return ""
    return c.correlation_id


def x_correlation_id__mutmut_2(obj: CoverBearer) -> str:
    """Get the correlation_id from a Cover-bearing type, or empty string if missing."""
    c = cover_of(None)
    if c is None:
        return ""
    return c.correlation_id


def x_correlation_id__mutmut_3(obj: CoverBearer) -> str:
    """Get the correlation_id from a Cover-bearing type, or empty string if missing."""
    c = cover_of(obj)
    if c is not None:
        return ""
    return c.correlation_id


def x_correlation_id__mutmut_4(obj: CoverBearer) -> str:
    """Get the correlation_id from a Cover-bearing type, or empty string if missing."""
    c = cover_of(obj)
    if c is None:
        return "XXXX"
    return c.correlation_id

x_correlation_id__mutmut_mutants : ClassVar[MutantDict] = {
'x_correlation_id__mutmut_1': x_correlation_id__mutmut_1, 
    'x_correlation_id__mutmut_2': x_correlation_id__mutmut_2, 
    'x_correlation_id__mutmut_3': x_correlation_id__mutmut_3, 
    'x_correlation_id__mutmut_4': x_correlation_id__mutmut_4
}

def correlation_id(*args, **kwargs):
    result = _mutmut_trampoline(x_correlation_id__mutmut_orig, x_correlation_id__mutmut_mutants, args, kwargs)
    return result 

correlation_id.__signature__ = _mutmut_signature(x_correlation_id__mutmut_orig)
x_correlation_id__mutmut_orig.__name__ = 'x_correlation_id'


def x_has_correlation_id__mutmut_orig(obj: CoverBearer) -> bool:
    """Return True if the correlation_id is present and non-empty."""
    return bool(correlation_id(obj))


def x_has_correlation_id__mutmut_1(obj: CoverBearer) -> bool:
    """Return True if the correlation_id is present and non-empty."""
    return bool(None)


def x_has_correlation_id__mutmut_2(obj: CoverBearer) -> bool:
    """Return True if the correlation_id is present and non-empty."""
    return bool(correlation_id(None))

x_has_correlation_id__mutmut_mutants : ClassVar[MutantDict] = {
'x_has_correlation_id__mutmut_1': x_has_correlation_id__mutmut_1, 
    'x_has_correlation_id__mutmut_2': x_has_correlation_id__mutmut_2
}

def has_correlation_id(*args, **kwargs):
    result = _mutmut_trampoline(x_has_correlation_id__mutmut_orig, x_has_correlation_id__mutmut_mutants, args, kwargs)
    return result 

has_correlation_id.__signature__ = _mutmut_signature(x_has_correlation_id__mutmut_orig)
x_has_correlation_id__mutmut_orig.__name__ = 'x_has_correlation_id'


def x_root_uuid__mutmut_orig(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(obj)
    if c is None or not c.HasField("root"):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def x_root_uuid__mutmut_1(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = None
    if c is None or not c.HasField("root"):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def x_root_uuid__mutmut_2(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(None)
    if c is None or not c.HasField("root"):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def x_root_uuid__mutmut_3(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(obj)
    if c is None and not c.HasField("root"):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def x_root_uuid__mutmut_4(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(obj)
    if c is not None or not c.HasField("root"):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def x_root_uuid__mutmut_5(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(obj)
    if c is None or c.HasField("root"):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def x_root_uuid__mutmut_6(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(obj)
    if c is None or not c.HasField(None):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def x_root_uuid__mutmut_7(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(obj)
    if c is None or not c.HasField("XXrootXX"):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def x_root_uuid__mutmut_8(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(obj)
    if c is None or not c.HasField("ROOT"):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def x_root_uuid__mutmut_9(obj: CoverBearer) -> Optional[PyUUID]:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(obj)
    if c is None or not c.HasField("root"):
        return None
    try:
        return PyUUID(bytes=None)
    except ValueError:
        return None

x_root_uuid__mutmut_mutants : ClassVar[MutantDict] = {
'x_root_uuid__mutmut_1': x_root_uuid__mutmut_1, 
    'x_root_uuid__mutmut_2': x_root_uuid__mutmut_2, 
    'x_root_uuid__mutmut_3': x_root_uuid__mutmut_3, 
    'x_root_uuid__mutmut_4': x_root_uuid__mutmut_4, 
    'x_root_uuid__mutmut_5': x_root_uuid__mutmut_5, 
    'x_root_uuid__mutmut_6': x_root_uuid__mutmut_6, 
    'x_root_uuid__mutmut_7': x_root_uuid__mutmut_7, 
    'x_root_uuid__mutmut_8': x_root_uuid__mutmut_8, 
    'x_root_uuid__mutmut_9': x_root_uuid__mutmut_9
}

def root_uuid(*args, **kwargs):
    result = _mutmut_trampoline(x_root_uuid__mutmut_orig, x_root_uuid__mutmut_mutants, args, kwargs)
    return result 

root_uuid.__signature__ = _mutmut_signature(x_root_uuid__mutmut_orig)
x_root_uuid__mutmut_orig.__name__ = 'x_root_uuid'


def x_root_id_hex__mutmut_orig(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(obj)
    if c is None or not c.HasField("root"):
        return ""
    return c.root.value.hex()


def x_root_id_hex__mutmut_1(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = None
    if c is None or not c.HasField("root"):
        return ""
    return c.root.value.hex()


def x_root_id_hex__mutmut_2(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(None)
    if c is None or not c.HasField("root"):
        return ""
    return c.root.value.hex()


def x_root_id_hex__mutmut_3(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(obj)
    if c is None and not c.HasField("root"):
        return ""
    return c.root.value.hex()


def x_root_id_hex__mutmut_4(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(obj)
    if c is not None or not c.HasField("root"):
        return ""
    return c.root.value.hex()


def x_root_id_hex__mutmut_5(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(obj)
    if c is None or c.HasField("root"):
        return ""
    return c.root.value.hex()


def x_root_id_hex__mutmut_6(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(obj)
    if c is None or not c.HasField(None):
        return ""
    return c.root.value.hex()


def x_root_id_hex__mutmut_7(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(obj)
    if c is None or not c.HasField("XXrootXX"):
        return ""
    return c.root.value.hex()


def x_root_id_hex__mutmut_8(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(obj)
    if c is None or not c.HasField("ROOT"):
        return ""
    return c.root.value.hex()


def x_root_id_hex__mutmut_9(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(obj)
    if c is None or not c.HasField("root"):
        return "XXXX"
    return c.root.value.hex()

x_root_id_hex__mutmut_mutants : ClassVar[MutantDict] = {
'x_root_id_hex__mutmut_1': x_root_id_hex__mutmut_1, 
    'x_root_id_hex__mutmut_2': x_root_id_hex__mutmut_2, 
    'x_root_id_hex__mutmut_3': x_root_id_hex__mutmut_3, 
    'x_root_id_hex__mutmut_4': x_root_id_hex__mutmut_4, 
    'x_root_id_hex__mutmut_5': x_root_id_hex__mutmut_5, 
    'x_root_id_hex__mutmut_6': x_root_id_hex__mutmut_6, 
    'x_root_id_hex__mutmut_7': x_root_id_hex__mutmut_7, 
    'x_root_id_hex__mutmut_8': x_root_id_hex__mutmut_8, 
    'x_root_id_hex__mutmut_9': x_root_id_hex__mutmut_9
}

def root_id_hex(*args, **kwargs):
    result = _mutmut_trampoline(x_root_id_hex__mutmut_orig, x_root_id_hex__mutmut_mutants, args, kwargs)
    return result 

root_id_hex.__signature__ = _mutmut_signature(x_root_id_hex__mutmut_orig)
x_root_id_hex__mutmut_orig.__name__ = 'x_root_id_hex'


def x_edition__mutmut_orig(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is None or not c.HasField("edition") or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_1(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = None
    if c is None or not c.HasField("edition") or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_2(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(None)
    if c is None or not c.HasField("edition") or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_3(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is None or not c.HasField("edition") and not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_4(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is None and not c.HasField("edition") or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_5(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is not None or not c.HasField("edition") or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_6(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is None or c.HasField("edition") or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_7(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is None or not c.HasField(None) or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_8(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is None or not c.HasField("XXeditionXX") or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_9(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is None or not c.HasField("EDITION") or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def x_edition__mutmut_10(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is None or not c.HasField("edition") or c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name

x_edition__mutmut_mutants : ClassVar[MutantDict] = {
'x_edition__mutmut_1': x_edition__mutmut_1, 
    'x_edition__mutmut_2': x_edition__mutmut_2, 
    'x_edition__mutmut_3': x_edition__mutmut_3, 
    'x_edition__mutmut_4': x_edition__mutmut_4, 
    'x_edition__mutmut_5': x_edition__mutmut_5, 
    'x_edition__mutmut_6': x_edition__mutmut_6, 
    'x_edition__mutmut_7': x_edition__mutmut_7, 
    'x_edition__mutmut_8': x_edition__mutmut_8, 
    'x_edition__mutmut_9': x_edition__mutmut_9, 
    'x_edition__mutmut_10': x_edition__mutmut_10
}

def edition(*args, **kwargs):
    result = _mutmut_trampoline(x_edition__mutmut_orig, x_edition__mutmut_mutants, args, kwargs)
    return result 

edition.__signature__ = _mutmut_signature(x_edition__mutmut_orig)
x_edition__mutmut_orig.__name__ = 'x_edition'


def x_edition_opt__mutmut_orig(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is None or not c.HasField("edition") or not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_1(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = None
    if c is None or not c.HasField("edition") or not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_2(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(None)
    if c is None or not c.HasField("edition") or not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_3(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is None or not c.HasField("edition") and not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_4(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is None and not c.HasField("edition") or not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_5(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is not None or not c.HasField("edition") or not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_6(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is None or c.HasField("edition") or not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_7(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is None or not c.HasField(None) or not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_8(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is None or not c.HasField("XXeditionXX") or not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_9(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is None or not c.HasField("EDITION") or not c.edition.name:
        return None
    return c.edition.name


def x_edition_opt__mutmut_10(obj: CoverBearer) -> Optional[str]:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is None or not c.HasField("edition") or c.edition.name:
        return None
    return c.edition.name

x_edition_opt__mutmut_mutants : ClassVar[MutantDict] = {
'x_edition_opt__mutmut_1': x_edition_opt__mutmut_1, 
    'x_edition_opt__mutmut_2': x_edition_opt__mutmut_2, 
    'x_edition_opt__mutmut_3': x_edition_opt__mutmut_3, 
    'x_edition_opt__mutmut_4': x_edition_opt__mutmut_4, 
    'x_edition_opt__mutmut_5': x_edition_opt__mutmut_5, 
    'x_edition_opt__mutmut_6': x_edition_opt__mutmut_6, 
    'x_edition_opt__mutmut_7': x_edition_opt__mutmut_7, 
    'x_edition_opt__mutmut_8': x_edition_opt__mutmut_8, 
    'x_edition_opt__mutmut_9': x_edition_opt__mutmut_9, 
    'x_edition_opt__mutmut_10': x_edition_opt__mutmut_10
}

def edition_opt(*args, **kwargs):
    result = _mutmut_trampoline(x_edition_opt__mutmut_orig, x_edition_opt__mutmut_mutants, args, kwargs)
    return result 

edition_opt.__signature__ = _mutmut_signature(x_edition_opt__mutmut_orig)
x_edition_opt__mutmut_orig.__name__ = 'x_edition_opt'


def x_routing_key__mutmut_orig(obj: CoverBearer) -> str:
    """Compute the bus routing key for a Cover-bearing type."""
    return domain(obj)


def x_routing_key__mutmut_1(obj: CoverBearer) -> str:
    """Compute the bus routing key for a Cover-bearing type."""
    return domain(None)

x_routing_key__mutmut_mutants : ClassVar[MutantDict] = {
'x_routing_key__mutmut_1': x_routing_key__mutmut_1
}

def routing_key(*args, **kwargs):
    result = _mutmut_trampoline(x_routing_key__mutmut_orig, x_routing_key__mutmut_mutants, args, kwargs)
    return result 

routing_key.__signature__ = _mutmut_signature(x_routing_key__mutmut_orig)
x_routing_key__mutmut_orig.__name__ = 'x_routing_key'


def x_cache_key__mutmut_orig(obj: CoverBearer) -> str:
    """Generate a cache key based on domain + root."""
    return f"{domain(obj)}:{root_id_hex(obj)}"


def x_cache_key__mutmut_1(obj: CoverBearer) -> str:
    """Generate a cache key based on domain + root."""
    return f"{domain(None)}:{root_id_hex(obj)}"


def x_cache_key__mutmut_2(obj: CoverBearer) -> str:
    """Generate a cache key based on domain + root."""
    return f"{domain(obj)}:{root_id_hex(None)}"

x_cache_key__mutmut_mutants : ClassVar[MutantDict] = {
'x_cache_key__mutmut_1': x_cache_key__mutmut_1, 
    'x_cache_key__mutmut_2': x_cache_key__mutmut_2
}

def cache_key(*args, **kwargs):
    result = _mutmut_trampoline(x_cache_key__mutmut_orig, x_cache_key__mutmut_mutants, args, kwargs)
    return result 

cache_key.__signature__ = _mutmut_signature(x_cache_key__mutmut_orig)
x_cache_key__mutmut_orig.__name__ = 'x_cache_key'


# UUID conversion


def x_uuid_to_proto__mutmut_orig(u: PyUUID) -> UUID:
    """Convert a Python UUID to a proto UUID."""
    return UUID(value=u.bytes)


# UUID conversion


def x_uuid_to_proto__mutmut_1(u: PyUUID) -> UUID:
    """Convert a Python UUID to a proto UUID."""
    return UUID(value=None)

x_uuid_to_proto__mutmut_mutants : ClassVar[MutantDict] = {
'x_uuid_to_proto__mutmut_1': x_uuid_to_proto__mutmut_1
}

def uuid_to_proto(*args, **kwargs):
    result = _mutmut_trampoline(x_uuid_to_proto__mutmut_orig, x_uuid_to_proto__mutmut_mutants, args, kwargs)
    return result 

uuid_to_proto.__signature__ = _mutmut_signature(x_uuid_to_proto__mutmut_orig)
x_uuid_to_proto__mutmut_orig.__name__ = 'x_uuid_to_proto'


def x_proto_to_uuid__mutmut_orig(u: UUID) -> PyUUID:
    """Convert a proto UUID to Python UUID."""
    return PyUUID(bytes=u.value)


def x_proto_to_uuid__mutmut_1(u: UUID) -> PyUUID:
    """Convert a proto UUID to Python UUID."""
    return PyUUID(bytes=None)

x_proto_to_uuid__mutmut_mutants : ClassVar[MutantDict] = {
'x_proto_to_uuid__mutmut_1': x_proto_to_uuid__mutmut_1
}

def proto_to_uuid(*args, **kwargs):
    result = _mutmut_trampoline(x_proto_to_uuid__mutmut_orig, x_proto_to_uuid__mutmut_mutants, args, kwargs)
    return result 

proto_to_uuid.__signature__ = _mutmut_signature(x_proto_to_uuid__mutmut_orig)
x_proto_to_uuid__mutmut_orig.__name__ = 'x_proto_to_uuid'


# Edition helpers


def x_main_timeline__mutmut_orig() -> Edition:
    """Return an Edition representing the main timeline."""
    return Edition(name=DEFAULT_EDITION)


# Edition helpers


def x_main_timeline__mutmut_1() -> Edition:
    """Return an Edition representing the main timeline."""
    return Edition(name=None)

x_main_timeline__mutmut_mutants : ClassVar[MutantDict] = {
'x_main_timeline__mutmut_1': x_main_timeline__mutmut_1
}

def main_timeline(*args, **kwargs):
    result = _mutmut_trampoline(x_main_timeline__mutmut_orig, x_main_timeline__mutmut_mutants, args, kwargs)
    return result 

main_timeline.__signature__ = _mutmut_signature(x_main_timeline__mutmut_orig)
x_main_timeline__mutmut_orig.__name__ = 'x_main_timeline'


def x_implicit_edition__mutmut_orig(name: str) -> Edition:
    """Create an edition with the given name but no divergences."""
    return Edition(name=name)


def x_implicit_edition__mutmut_1(name: str) -> Edition:
    """Create an edition with the given name but no divergences."""
    return Edition(name=None)

x_implicit_edition__mutmut_mutants : ClassVar[MutantDict] = {
'x_implicit_edition__mutmut_1': x_implicit_edition__mutmut_1
}

def implicit_edition(*args, **kwargs):
    result = _mutmut_trampoline(x_implicit_edition__mutmut_orig, x_implicit_edition__mutmut_mutants, args, kwargs)
    return result 

implicit_edition.__signature__ = _mutmut_signature(x_implicit_edition__mutmut_orig)
x_implicit_edition__mutmut_orig.__name__ = 'x_implicit_edition'


def x_explicit_edition__mutmut_orig(name: str, divergences: list[DomainDivergence]) -> Edition:
    """Create an edition with divergence points."""
    return Edition(name=name, divergences=divergences)


def x_explicit_edition__mutmut_1(name: str, divergences: list[DomainDivergence]) -> Edition:
    """Create an edition with divergence points."""
    return Edition(name=None, divergences=divergences)


def x_explicit_edition__mutmut_2(name: str, divergences: list[DomainDivergence]) -> Edition:
    """Create an edition with divergence points."""
    return Edition(name=name, divergences=None)


def x_explicit_edition__mutmut_3(name: str, divergences: list[DomainDivergence]) -> Edition:
    """Create an edition with divergence points."""
    return Edition(divergences=divergences)


def x_explicit_edition__mutmut_4(name: str, divergences: list[DomainDivergence]) -> Edition:
    """Create an edition with divergence points."""
    return Edition(name=name, )

x_explicit_edition__mutmut_mutants : ClassVar[MutantDict] = {
'x_explicit_edition__mutmut_1': x_explicit_edition__mutmut_1, 
    'x_explicit_edition__mutmut_2': x_explicit_edition__mutmut_2, 
    'x_explicit_edition__mutmut_3': x_explicit_edition__mutmut_3, 
    'x_explicit_edition__mutmut_4': x_explicit_edition__mutmut_4
}

def explicit_edition(*args, **kwargs):
    result = _mutmut_trampoline(x_explicit_edition__mutmut_orig, x_explicit_edition__mutmut_mutants, args, kwargs)
    return result 

explicit_edition.__signature__ = _mutmut_signature(x_explicit_edition__mutmut_orig)
x_explicit_edition__mutmut_orig.__name__ = 'x_explicit_edition'


def x_is_main_timeline__mutmut_orig(e: Optional[Edition]) -> bool:
    """Check if an edition represents the main timeline."""
    return e is None or not e.name or e.name == DEFAULT_EDITION


def x_is_main_timeline__mutmut_1(e: Optional[Edition]) -> bool:
    """Check if an edition represents the main timeline."""
    return e is None or not e.name and e.name == DEFAULT_EDITION


def x_is_main_timeline__mutmut_2(e: Optional[Edition]) -> bool:
    """Check if an edition represents the main timeline."""
    return e is None and not e.name or e.name == DEFAULT_EDITION


def x_is_main_timeline__mutmut_3(e: Optional[Edition]) -> bool:
    """Check if an edition represents the main timeline."""
    return e is not None or not e.name or e.name == DEFAULT_EDITION


def x_is_main_timeline__mutmut_4(e: Optional[Edition]) -> bool:
    """Check if an edition represents the main timeline."""
    return e is None or e.name or e.name == DEFAULT_EDITION


def x_is_main_timeline__mutmut_5(e: Optional[Edition]) -> bool:
    """Check if an edition represents the main timeline."""
    return e is None or not e.name or e.name != DEFAULT_EDITION

x_is_main_timeline__mutmut_mutants : ClassVar[MutantDict] = {
'x_is_main_timeline__mutmut_1': x_is_main_timeline__mutmut_1, 
    'x_is_main_timeline__mutmut_2': x_is_main_timeline__mutmut_2, 
    'x_is_main_timeline__mutmut_3': x_is_main_timeline__mutmut_3, 
    'x_is_main_timeline__mutmut_4': x_is_main_timeline__mutmut_4, 
    'x_is_main_timeline__mutmut_5': x_is_main_timeline__mutmut_5
}

def is_main_timeline(*args, **kwargs):
    result = _mutmut_trampoline(x_is_main_timeline__mutmut_orig, x_is_main_timeline__mutmut_mutants, args, kwargs)
    return result 

is_main_timeline.__signature__ = _mutmut_signature(x_is_main_timeline__mutmut_orig)
x_is_main_timeline__mutmut_orig.__name__ = 'x_is_main_timeline'


def x_divergence_for__mutmut_orig(e: Optional[Edition], domain_name: str) -> int:
    """Return the divergence sequence for a domain, or -1 if not found."""
    if e is None:
        return -1
    for d in e.divergences:
        if d.domain == domain_name:
            return d.sequence
    return -1


def x_divergence_for__mutmut_1(e: Optional[Edition], domain_name: str) -> int:
    """Return the divergence sequence for a domain, or -1 if not found."""
    if e is not None:
        return -1
    for d in e.divergences:
        if d.domain == domain_name:
            return d.sequence
    return -1


def x_divergence_for__mutmut_2(e: Optional[Edition], domain_name: str) -> int:
    """Return the divergence sequence for a domain, or -1 if not found."""
    if e is None:
        return +1
    for d in e.divergences:
        if d.domain == domain_name:
            return d.sequence
    return -1


def x_divergence_for__mutmut_3(e: Optional[Edition], domain_name: str) -> int:
    """Return the divergence sequence for a domain, or -1 if not found."""
    if e is None:
        return -2
    for d in e.divergences:
        if d.domain == domain_name:
            return d.sequence
    return -1


def x_divergence_for__mutmut_4(e: Optional[Edition], domain_name: str) -> int:
    """Return the divergence sequence for a domain, or -1 if not found."""
    if e is None:
        return -1
    for d in e.divergences:
        if d.domain != domain_name:
            return d.sequence
    return -1


def x_divergence_for__mutmut_5(e: Optional[Edition], domain_name: str) -> int:
    """Return the divergence sequence for a domain, or -1 if not found."""
    if e is None:
        return -1
    for d in e.divergences:
        if d.domain == domain_name:
            return d.sequence
    return +1


def x_divergence_for__mutmut_6(e: Optional[Edition], domain_name: str) -> int:
    """Return the divergence sequence for a domain, or -1 if not found."""
    if e is None:
        return -1
    for d in e.divergences:
        if d.domain == domain_name:
            return d.sequence
    return -2

x_divergence_for__mutmut_mutants : ClassVar[MutantDict] = {
'x_divergence_for__mutmut_1': x_divergence_for__mutmut_1, 
    'x_divergence_for__mutmut_2': x_divergence_for__mutmut_2, 
    'x_divergence_for__mutmut_3': x_divergence_for__mutmut_3, 
    'x_divergence_for__mutmut_4': x_divergence_for__mutmut_4, 
    'x_divergence_for__mutmut_5': x_divergence_for__mutmut_5, 
    'x_divergence_for__mutmut_6': x_divergence_for__mutmut_6
}

def divergence_for(*args, **kwargs):
    result = _mutmut_trampoline(x_divergence_for__mutmut_orig, x_divergence_for__mutmut_mutants, args, kwargs)
    return result 

divergence_for.__signature__ = _mutmut_signature(x_divergence_for__mutmut_orig)
x_divergence_for__mutmut_orig.__name__ = 'x_divergence_for'


# EventBook helpers


def x_next_sequence__mutmut_orig(book: EventBook) -> int:
    """Return the next sequence number from an EventBook.

    The framework computes this value on load.
    """
    if book is None:
        return 0
    return book.next_sequence


# EventBook helpers


def x_next_sequence__mutmut_1(book: EventBook) -> int:
    """Return the next sequence number from an EventBook.

    The framework computes this value on load.
    """
    if book is not None:
        return 0
    return book.next_sequence


# EventBook helpers


def x_next_sequence__mutmut_2(book: EventBook) -> int:
    """Return the next sequence number from an EventBook.

    The framework computes this value on load.
    """
    if book is None:
        return 1
    return book.next_sequence

x_next_sequence__mutmut_mutants : ClassVar[MutantDict] = {
'x_next_sequence__mutmut_1': x_next_sequence__mutmut_1, 
    'x_next_sequence__mutmut_2': x_next_sequence__mutmut_2
}

def next_sequence(*args, **kwargs):
    result = _mutmut_trampoline(x_next_sequence__mutmut_orig, x_next_sequence__mutmut_mutants, args, kwargs)
    return result 

next_sequence.__signature__ = _mutmut_signature(x_next_sequence__mutmut_orig)
x_next_sequence__mutmut_orig.__name__ = 'x_next_sequence'


def x_event_pages__mutmut_orig(book: Optional[EventBook]) -> list[EventPage]:
    """Return the event pages from an EventBook, or empty list if None."""
    if book is None:
        return []
    return list(book.pages)


def x_event_pages__mutmut_1(book: Optional[EventBook]) -> list[EventPage]:
    """Return the event pages from an EventBook, or empty list if None."""
    if book is not None:
        return []
    return list(book.pages)


def x_event_pages__mutmut_2(book: Optional[EventBook]) -> list[EventPage]:
    """Return the event pages from an EventBook, or empty list if None."""
    if book is None:
        return []
    return list(None)

x_event_pages__mutmut_mutants : ClassVar[MutantDict] = {
'x_event_pages__mutmut_1': x_event_pages__mutmut_1, 
    'x_event_pages__mutmut_2': x_event_pages__mutmut_2
}

def event_pages(*args, **kwargs):
    result = _mutmut_trampoline(x_event_pages__mutmut_orig, x_event_pages__mutmut_mutants, args, kwargs)
    return result 

event_pages.__signature__ = _mutmut_signature(x_event_pages__mutmut_orig)
x_event_pages__mutmut_orig.__name__ = 'x_event_pages'


# CommandBook helpers


def x_command_pages__mutmut_orig(book: Optional[CommandBook]) -> list[CommandPage]:
    """Return the command pages from a CommandBook, or empty list if None."""
    if book is None:
        return []
    return list(book.pages)


# CommandBook helpers


def x_command_pages__mutmut_1(book: Optional[CommandBook]) -> list[CommandPage]:
    """Return the command pages from a CommandBook, or empty list if None."""
    if book is not None:
        return []
    return list(book.pages)


# CommandBook helpers


def x_command_pages__mutmut_2(book: Optional[CommandBook]) -> list[CommandPage]:
    """Return the command pages from a CommandBook, or empty list if None."""
    if book is None:
        return []
    return list(None)

x_command_pages__mutmut_mutants : ClassVar[MutantDict] = {
'x_command_pages__mutmut_1': x_command_pages__mutmut_1, 
    'x_command_pages__mutmut_2': x_command_pages__mutmut_2
}

def command_pages(*args, **kwargs):
    result = _mutmut_trampoline(x_command_pages__mutmut_orig, x_command_pages__mutmut_mutants, args, kwargs)
    return result 

command_pages.__signature__ = _mutmut_signature(x_command_pages__mutmut_orig)
x_command_pages__mutmut_orig.__name__ = 'x_command_pages'


# CommandResponse helpers


def x_events_from_response__mutmut_orig(resp) -> list[EventPage]:
    """Extract the event pages from a CommandResponse."""
    if resp is None or not resp.HasField("events"):
        return []
    return list(resp.events.pages)


# CommandResponse helpers


def x_events_from_response__mutmut_1(resp) -> list[EventPage]:
    """Extract the event pages from a CommandResponse."""
    if resp is None and not resp.HasField("events"):
        return []
    return list(resp.events.pages)


# CommandResponse helpers


def x_events_from_response__mutmut_2(resp) -> list[EventPage]:
    """Extract the event pages from a CommandResponse."""
    if resp is not None or not resp.HasField("events"):
        return []
    return list(resp.events.pages)


# CommandResponse helpers


def x_events_from_response__mutmut_3(resp) -> list[EventPage]:
    """Extract the event pages from a CommandResponse."""
    if resp is None or resp.HasField("events"):
        return []
    return list(resp.events.pages)


# CommandResponse helpers


def x_events_from_response__mutmut_4(resp) -> list[EventPage]:
    """Extract the event pages from a CommandResponse."""
    if resp is None or not resp.HasField(None):
        return []
    return list(resp.events.pages)


# CommandResponse helpers


def x_events_from_response__mutmut_5(resp) -> list[EventPage]:
    """Extract the event pages from a CommandResponse."""
    if resp is None or not resp.HasField("XXeventsXX"):
        return []
    return list(resp.events.pages)


# CommandResponse helpers


def x_events_from_response__mutmut_6(resp) -> list[EventPage]:
    """Extract the event pages from a CommandResponse."""
    if resp is None or not resp.HasField("EVENTS"):
        return []
    return list(resp.events.pages)


# CommandResponse helpers


def x_events_from_response__mutmut_7(resp) -> list[EventPage]:
    """Extract the event pages from a CommandResponse."""
    if resp is None or not resp.HasField("events"):
        return []
    return list(None)

x_events_from_response__mutmut_mutants : ClassVar[MutantDict] = {
'x_events_from_response__mutmut_1': x_events_from_response__mutmut_1, 
    'x_events_from_response__mutmut_2': x_events_from_response__mutmut_2, 
    'x_events_from_response__mutmut_3': x_events_from_response__mutmut_3, 
    'x_events_from_response__mutmut_4': x_events_from_response__mutmut_4, 
    'x_events_from_response__mutmut_5': x_events_from_response__mutmut_5, 
    'x_events_from_response__mutmut_6': x_events_from_response__mutmut_6, 
    'x_events_from_response__mutmut_7': x_events_from_response__mutmut_7
}

def events_from_response(*args, **kwargs):
    result = _mutmut_trampoline(x_events_from_response__mutmut_orig, x_events_from_response__mutmut_mutants, args, kwargs)
    return result 

events_from_response.__signature__ = _mutmut_signature(x_events_from_response__mutmut_orig)
x_events_from_response__mutmut_orig.__name__ = 'x_events_from_response'


# Type URL helpers


def type_url(package_name: str, type_name: str) -> str:
    """Construct a full type URL from a package and type name."""
    return f"{TYPE_URL_PREFIX}{package_name}.{type_name}"


def x_type_name_from_url__mutmut_orig(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_1(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "XX.XX" in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_2(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." not in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_3(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(None, 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_4(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", None)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_5(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_6(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", )[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_7(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.split(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_8(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit("XX.XX", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_9(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 2)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_10(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[2]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_11(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "XX/XX" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_12(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" not in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_13(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit(None, 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_14(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", None)[1]
    return type_url_str


def x_type_name_from_url__mutmut_15(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit(1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_16(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", )[1]
    return type_url_str


def x_type_name_from_url__mutmut_17(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.split("/", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_18(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("XX/XX", 1)[1]
    return type_url_str


def x_type_name_from_url__mutmut_19(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 2)[1]
    return type_url_str


def x_type_name_from_url__mutmut_20(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[2]
    return type_url_str

x_type_name_from_url__mutmut_mutants : ClassVar[MutantDict] = {
'x_type_name_from_url__mutmut_1': x_type_name_from_url__mutmut_1, 
    'x_type_name_from_url__mutmut_2': x_type_name_from_url__mutmut_2, 
    'x_type_name_from_url__mutmut_3': x_type_name_from_url__mutmut_3, 
    'x_type_name_from_url__mutmut_4': x_type_name_from_url__mutmut_4, 
    'x_type_name_from_url__mutmut_5': x_type_name_from_url__mutmut_5, 
    'x_type_name_from_url__mutmut_6': x_type_name_from_url__mutmut_6, 
    'x_type_name_from_url__mutmut_7': x_type_name_from_url__mutmut_7, 
    'x_type_name_from_url__mutmut_8': x_type_name_from_url__mutmut_8, 
    'x_type_name_from_url__mutmut_9': x_type_name_from_url__mutmut_9, 
    'x_type_name_from_url__mutmut_10': x_type_name_from_url__mutmut_10, 
    'x_type_name_from_url__mutmut_11': x_type_name_from_url__mutmut_11, 
    'x_type_name_from_url__mutmut_12': x_type_name_from_url__mutmut_12, 
    'x_type_name_from_url__mutmut_13': x_type_name_from_url__mutmut_13, 
    'x_type_name_from_url__mutmut_14': x_type_name_from_url__mutmut_14, 
    'x_type_name_from_url__mutmut_15': x_type_name_from_url__mutmut_15, 
    'x_type_name_from_url__mutmut_16': x_type_name_from_url__mutmut_16, 
    'x_type_name_from_url__mutmut_17': x_type_name_from_url__mutmut_17, 
    'x_type_name_from_url__mutmut_18': x_type_name_from_url__mutmut_18, 
    'x_type_name_from_url__mutmut_19': x_type_name_from_url__mutmut_19, 
    'x_type_name_from_url__mutmut_20': x_type_name_from_url__mutmut_20
}

def type_name_from_url(*args, **kwargs):
    result = _mutmut_trampoline(x_type_name_from_url__mutmut_orig, x_type_name_from_url__mutmut_mutants, args, kwargs)
    return result 

type_name_from_url.__signature__ = _mutmut_signature(x_type_name_from_url__mutmut_orig)
x_type_name_from_url__mutmut_orig.__name__ = 'x_type_name_from_url'


def x_type_url_matches__mutmut_orig(type_url_str: str, suffix: str) -> bool:
    """Check if a type URL ends with the given suffix."""
    return type_url_str.endswith(suffix)


def x_type_url_matches__mutmut_1(type_url_str: str, suffix: str) -> bool:
    """Check if a type URL ends with the given suffix."""
    return type_url_str.endswith(None)

x_type_url_matches__mutmut_mutants : ClassVar[MutantDict] = {
'x_type_url_matches__mutmut_1': x_type_url_matches__mutmut_1
}

def type_url_matches(*args, **kwargs):
    result = _mutmut_trampoline(x_type_url_matches__mutmut_orig, x_type_url_matches__mutmut_mutants, args, kwargs)
    return result 

type_url_matches.__signature__ = _mutmut_signature(x_type_url_matches__mutmut_orig)
x_type_url_matches__mutmut_orig.__name__ = 'x_type_url_matches'


# Timestamp helpers


def x_now__mutmut_orig() -> Timestamp:
    """Return the current time as a protobuf Timestamp."""
    ts = Timestamp()
    ts.GetCurrentTime()
    return ts


# Timestamp helpers


def x_now__mutmut_1() -> Timestamp:
    """Return the current time as a protobuf Timestamp."""
    ts = None
    ts.GetCurrentTime()
    return ts

x_now__mutmut_mutants : ClassVar[MutantDict] = {
'x_now__mutmut_1': x_now__mutmut_1
}

def now(*args, **kwargs):
    result = _mutmut_trampoline(x_now__mutmut_orig, x_now__mutmut_mutants, args, kwargs)
    return result 

now.__signature__ = _mutmut_signature(x_now__mutmut_orig)
x_now__mutmut_orig.__name__ = 'x_now'


def x_parse_timestamp__mutmut_orig(rfc3339: str) -> Timestamp:
    """Parse an RFC3339 timestamp string."""
    try:
        ts = Timestamp()
        ts.FromJsonString(rfc3339)
        return ts
    except ValueError as e:
        raise InvalidTimestampError(str(e)) from e


def x_parse_timestamp__mutmut_1(rfc3339: str) -> Timestamp:
    """Parse an RFC3339 timestamp string."""
    try:
        ts = None
        ts.FromJsonString(rfc3339)
        return ts
    except ValueError as e:
        raise InvalidTimestampError(str(e)) from e


def x_parse_timestamp__mutmut_2(rfc3339: str) -> Timestamp:
    """Parse an RFC3339 timestamp string."""
    try:
        ts = Timestamp()
        ts.FromJsonString(None)
        return ts
    except ValueError as e:
        raise InvalidTimestampError(str(e)) from e


def x_parse_timestamp__mutmut_3(rfc3339: str) -> Timestamp:
    """Parse an RFC3339 timestamp string."""
    try:
        ts = Timestamp()
        ts.FromJsonString(rfc3339)
        return ts
    except ValueError as e:
        raise InvalidTimestampError(None) from e


def x_parse_timestamp__mutmut_4(rfc3339: str) -> Timestamp:
    """Parse an RFC3339 timestamp string."""
    try:
        ts = Timestamp()
        ts.FromJsonString(rfc3339)
        return ts
    except ValueError as e:
        raise InvalidTimestampError(str(None)) from e

x_parse_timestamp__mutmut_mutants : ClassVar[MutantDict] = {
'x_parse_timestamp__mutmut_1': x_parse_timestamp__mutmut_1, 
    'x_parse_timestamp__mutmut_2': x_parse_timestamp__mutmut_2, 
    'x_parse_timestamp__mutmut_3': x_parse_timestamp__mutmut_3, 
    'x_parse_timestamp__mutmut_4': x_parse_timestamp__mutmut_4
}

def parse_timestamp(*args, **kwargs):
    result = _mutmut_trampoline(x_parse_timestamp__mutmut_orig, x_parse_timestamp__mutmut_mutants, args, kwargs)
    return result 

parse_timestamp.__signature__ = _mutmut_signature(x_parse_timestamp__mutmut_orig)
x_parse_timestamp__mutmut_orig.__name__ = 'x_parse_timestamp'


# Event decoding


def x_decode_event__mutmut_orig(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("event"):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_1(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None and not page.HasField("event"):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_2(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is not None or not page.HasField("event"):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_3(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or page.HasField("event"):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_4(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField(None):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_5(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("XXeventXX"):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_6(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("EVENT"):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_7(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("event"):
        return None
    if type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_8(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("event"):
        return None
    if not type_url_matches(None, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_9(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("event"):
        return None
    if not type_url_matches(page.event.type_url, None):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_10(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("event"):
        return None
    if not type_url_matches(type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_11(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("event"):
        return None
    if not type_url_matches(page.event.type_url, ):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_12(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("event"):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = None
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Event decoding


def x_decode_event__mutmut_13(page: EventPage, type_suffix: str, msg_class) -> Optional[object]:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("event"):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(None)
        return msg
    except Exception:
        return None

x_decode_event__mutmut_mutants : ClassVar[MutantDict] = {
'x_decode_event__mutmut_1': x_decode_event__mutmut_1, 
    'x_decode_event__mutmut_2': x_decode_event__mutmut_2, 
    'x_decode_event__mutmut_3': x_decode_event__mutmut_3, 
    'x_decode_event__mutmut_4': x_decode_event__mutmut_4, 
    'x_decode_event__mutmut_5': x_decode_event__mutmut_5, 
    'x_decode_event__mutmut_6': x_decode_event__mutmut_6, 
    'x_decode_event__mutmut_7': x_decode_event__mutmut_7, 
    'x_decode_event__mutmut_8': x_decode_event__mutmut_8, 
    'x_decode_event__mutmut_9': x_decode_event__mutmut_9, 
    'x_decode_event__mutmut_10': x_decode_event__mutmut_10, 
    'x_decode_event__mutmut_11': x_decode_event__mutmut_11, 
    'x_decode_event__mutmut_12': x_decode_event__mutmut_12, 
    'x_decode_event__mutmut_13': x_decode_event__mutmut_13
}

def decode_event(*args, **kwargs):
    result = _mutmut_trampoline(x_decode_event__mutmut_orig, x_decode_event__mutmut_mutants, args, kwargs)
    return result 

decode_event.__signature__ = _mutmut_signature(x_decode_event__mutmut_orig)
x_decode_event__mutmut_orig.__name__ = 'x_decode_event'


# Construction helpers


def x_new_cover__mutmut_orig(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        root=uuid_to_proto(root),
        correlation_id=correlation_id_val,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_1(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "XXXX",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        root=uuid_to_proto(root),
        correlation_id=correlation_id_val,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_2(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = None
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_3(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=None,
        root=uuid_to_proto(root),
        correlation_id=correlation_id_val,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_4(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        root=None,
        correlation_id=correlation_id_val,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_5(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        root=uuid_to_proto(root),
        correlation_id=None,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_6(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        root=uuid_to_proto(root),
        correlation_id=correlation_id_val,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_7(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        correlation_id=correlation_id_val,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_8(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        root=uuid_to_proto(root),
        )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_9(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        root=uuid_to_proto(None),
        correlation_id=correlation_id_val,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_10(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        root=uuid_to_proto(root),
        correlation_id=correlation_id_val,
    )
    if edition_val is None:
        cover.edition.CopyFrom(edition_val)
    return cover


# Construction helpers


def x_new_cover__mutmut_11(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Optional[Edition] = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        root=uuid_to_proto(root),
        correlation_id=correlation_id_val,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(None)
    return cover

x_new_cover__mutmut_mutants : ClassVar[MutantDict] = {
'x_new_cover__mutmut_1': x_new_cover__mutmut_1, 
    'x_new_cover__mutmut_2': x_new_cover__mutmut_2, 
    'x_new_cover__mutmut_3': x_new_cover__mutmut_3, 
    'x_new_cover__mutmut_4': x_new_cover__mutmut_4, 
    'x_new_cover__mutmut_5': x_new_cover__mutmut_5, 
    'x_new_cover__mutmut_6': x_new_cover__mutmut_6, 
    'x_new_cover__mutmut_7': x_new_cover__mutmut_7, 
    'x_new_cover__mutmut_8': x_new_cover__mutmut_8, 
    'x_new_cover__mutmut_9': x_new_cover__mutmut_9, 
    'x_new_cover__mutmut_10': x_new_cover__mutmut_10, 
    'x_new_cover__mutmut_11': x_new_cover__mutmut_11
}

def new_cover(*args, **kwargs):
    result = _mutmut_trampoline(x_new_cover__mutmut_orig, x_new_cover__mutmut_mutants, args, kwargs)
    return result 

new_cover.__signature__ = _mutmut_signature(x_new_cover__mutmut_orig)
x_new_cover__mutmut_orig.__name__ = 'x_new_cover'


def x_new_command_page__mutmut_orig(sequence: int, command: ProtoAny) -> CommandPage:
    """Create a command page from a sequence and Any message."""
    page = CommandPage(sequence=sequence)
    page.command.CopyFrom(command)
    return page


def x_new_command_page__mutmut_1(sequence: int, command: ProtoAny) -> CommandPage:
    """Create a command page from a sequence and Any message."""
    page = None
    page.command.CopyFrom(command)
    return page


def x_new_command_page__mutmut_2(sequence: int, command: ProtoAny) -> CommandPage:
    """Create a command page from a sequence and Any message."""
    page = CommandPage(sequence=None)
    page.command.CopyFrom(command)
    return page


def x_new_command_page__mutmut_3(sequence: int, command: ProtoAny) -> CommandPage:
    """Create a command page from a sequence and Any message."""
    page = CommandPage(sequence=sequence)
    page.command.CopyFrom(None)
    return page

x_new_command_page__mutmut_mutants : ClassVar[MutantDict] = {
'x_new_command_page__mutmut_1': x_new_command_page__mutmut_1, 
    'x_new_command_page__mutmut_2': x_new_command_page__mutmut_2, 
    'x_new_command_page__mutmut_3': x_new_command_page__mutmut_3
}

def new_command_page(*args, **kwargs):
    result = _mutmut_trampoline(x_new_command_page__mutmut_orig, x_new_command_page__mutmut_mutants, args, kwargs)
    return result 

new_command_page.__signature__ = _mutmut_signature(x_new_command_page__mutmut_orig)
x_new_command_page__mutmut_orig.__name__ = 'x_new_command_page'


def x_new_command_book__mutmut_orig(cover: Cover, pages: list[CommandPage]) -> CommandBook:
    """Create a CommandBook with the given cover and pages."""
    book = CommandBook()
    book.cover.CopyFrom(cover)
    book.pages.extend(pages)
    return book


def x_new_command_book__mutmut_1(cover: Cover, pages: list[CommandPage]) -> CommandBook:
    """Create a CommandBook with the given cover and pages."""
    book = None
    book.cover.CopyFrom(cover)
    book.pages.extend(pages)
    return book


def x_new_command_book__mutmut_2(cover: Cover, pages: list[CommandPage]) -> CommandBook:
    """Create a CommandBook with the given cover and pages."""
    book = CommandBook()
    book.cover.CopyFrom(None)
    book.pages.extend(pages)
    return book


def x_new_command_book__mutmut_3(cover: Cover, pages: list[CommandPage]) -> CommandBook:
    """Create a CommandBook with the given cover and pages."""
    book = CommandBook()
    book.cover.CopyFrom(cover)
    book.pages.extend(None)
    return book

x_new_command_book__mutmut_mutants : ClassVar[MutantDict] = {
'x_new_command_book__mutmut_1': x_new_command_book__mutmut_1, 
    'x_new_command_book__mutmut_2': x_new_command_book__mutmut_2, 
    'x_new_command_book__mutmut_3': x_new_command_book__mutmut_3
}

def new_command_book(*args, **kwargs):
    result = _mutmut_trampoline(x_new_command_book__mutmut_orig, x_new_command_book__mutmut_mutants, args, kwargs)
    return result 

new_command_book.__signature__ = _mutmut_signature(x_new_command_book__mutmut_orig)
x_new_command_book__mutmut_orig.__name__ = 'x_new_command_book'


def x_range_selection__mutmut_orig(lower: int, upper: Optional[int] = None) -> SequenceRange:
    """Create a sequence range selection."""
    r = SequenceRange(lower=lower)
    if upper is not None:
        r.upper = upper
    return r


def x_range_selection__mutmut_1(lower: int, upper: Optional[int] = None) -> SequenceRange:
    """Create a sequence range selection."""
    r = None
    if upper is not None:
        r.upper = upper
    return r


def x_range_selection__mutmut_2(lower: int, upper: Optional[int] = None) -> SequenceRange:
    """Create a sequence range selection."""
    r = SequenceRange(lower=None)
    if upper is not None:
        r.upper = upper
    return r


def x_range_selection__mutmut_3(lower: int, upper: Optional[int] = None) -> SequenceRange:
    """Create a sequence range selection."""
    r = SequenceRange(lower=lower)
    if upper is None:
        r.upper = upper
    return r


def x_range_selection__mutmut_4(lower: int, upper: Optional[int] = None) -> SequenceRange:
    """Create a sequence range selection."""
    r = SequenceRange(lower=lower)
    if upper is not None:
        r.upper = None
    return r

x_range_selection__mutmut_mutants : ClassVar[MutantDict] = {
'x_range_selection__mutmut_1': x_range_selection__mutmut_1, 
    'x_range_selection__mutmut_2': x_range_selection__mutmut_2, 
    'x_range_selection__mutmut_3': x_range_selection__mutmut_3, 
    'x_range_selection__mutmut_4': x_range_selection__mutmut_4
}

def range_selection(*args, **kwargs):
    result = _mutmut_trampoline(x_range_selection__mutmut_orig, x_range_selection__mutmut_mutants, args, kwargs)
    return result 

range_selection.__signature__ = _mutmut_signature(x_range_selection__mutmut_orig)
x_range_selection__mutmut_orig.__name__ = 'x_range_selection'


def x_temporal_by_sequence__mutmut_orig(seq: int) -> TemporalQuery:
    """Create a temporal selection as-of a sequence."""
    return TemporalQuery(as_of_sequence=seq)


def x_temporal_by_sequence__mutmut_1(seq: int) -> TemporalQuery:
    """Create a temporal selection as-of a sequence."""
    return TemporalQuery(as_of_sequence=None)

x_temporal_by_sequence__mutmut_mutants : ClassVar[MutantDict] = {
'x_temporal_by_sequence__mutmut_1': x_temporal_by_sequence__mutmut_1
}

def temporal_by_sequence(*args, **kwargs):
    result = _mutmut_trampoline(x_temporal_by_sequence__mutmut_orig, x_temporal_by_sequence__mutmut_mutants, args, kwargs)
    return result 

temporal_by_sequence.__signature__ = _mutmut_signature(x_temporal_by_sequence__mutmut_orig)
x_temporal_by_sequence__mutmut_orig.__name__ = 'x_temporal_by_sequence'


def x_temporal_by_time__mutmut_orig(ts: Timestamp) -> TemporalQuery:
    """Create a temporal selection as-of a timestamp."""
    tq = TemporalQuery()
    tq.as_of_time.CopyFrom(ts)
    return tq


def x_temporal_by_time__mutmut_1(ts: Timestamp) -> TemporalQuery:
    """Create a temporal selection as-of a timestamp."""
    tq = None
    tq.as_of_time.CopyFrom(ts)
    return tq


def x_temporal_by_time__mutmut_2(ts: Timestamp) -> TemporalQuery:
    """Create a temporal selection as-of a timestamp."""
    tq = TemporalQuery()
    tq.as_of_time.CopyFrom(None)
    return tq

x_temporal_by_time__mutmut_mutants : ClassVar[MutantDict] = {
'x_temporal_by_time__mutmut_1': x_temporal_by_time__mutmut_1, 
    'x_temporal_by_time__mutmut_2': x_temporal_by_time__mutmut_2
}

def temporal_by_time(*args, **kwargs):
    result = _mutmut_trampoline(x_temporal_by_time__mutmut_orig, x_temporal_by_time__mutmut_mutants, args, kwargs)
    return result 

temporal_by_time.__signature__ = _mutmut_signature(x_temporal_by_time__mutmut_orig)
x_temporal_by_time__mutmut_orig.__name__ = 'x_temporal_by_time'
