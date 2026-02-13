"""Wrapper classes for Angzarr proto types.

Each wrapper takes a protobuf message in its constructor and provides
extension methods as instance methods.
"""

from typing import Optional, List, Type, TypeVar
from uuid import UUID as PyUUID

from .proto.angzarr import (
    Cover,
    EventBook,
    CommandBook,
    Query,
    EventPage,
    CommandPage,
    CommandResponse,
)
from .helpers import (
    UNKNOWN_DOMAIN,
    DEFAULT_EDITION,
    type_url_matches,
)

T = TypeVar("T")
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


class CoverW:
    """Wrapper for Cover proto with extension methods."""

    def xǁCoverWǁ__init____mutmut_orig(self, proto: Cover) -> None:
        self.proto = proto

    def xǁCoverWǁ__init____mutmut_1(self, proto: Cover) -> None:
        self.proto = None
    
    xǁCoverWǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCoverWǁ__init____mutmut_1': xǁCoverWǁ__init____mutmut_1
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCoverWǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁCoverWǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁCoverWǁ__init____mutmut_orig)
    xǁCoverWǁ__init____mutmut_orig.__name__ = 'xǁCoverWǁ__init__'

    def xǁCoverWǁdomain__mutmut_orig(self) -> str:
        """Get the domain, or UNKNOWN_DOMAIN if missing."""
        if not self.proto.domain:
            return UNKNOWN_DOMAIN
        return self.proto.domain

    def xǁCoverWǁdomain__mutmut_1(self) -> str:
        """Get the domain, or UNKNOWN_DOMAIN if missing."""
        if self.proto.domain:
            return UNKNOWN_DOMAIN
        return self.proto.domain
    
    xǁCoverWǁdomain__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCoverWǁdomain__mutmut_1': xǁCoverWǁdomain__mutmut_1
    }
    
    def domain(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCoverWǁdomain__mutmut_orig"), object.__getattribute__(self, "xǁCoverWǁdomain__mutmut_mutants"), args, kwargs, self)
        return result 
    
    domain.__signature__ = _mutmut_signature(xǁCoverWǁdomain__mutmut_orig)
    xǁCoverWǁdomain__mutmut_orig.__name__ = 'xǁCoverWǁdomain'

    def correlation_id(self) -> str:
        """Get the correlation_id, or empty string if missing."""
        return self.proto.correlation_id

    def xǁCoverWǁhas_correlation_id__mutmut_orig(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(self.proto.correlation_id)

    def xǁCoverWǁhas_correlation_id__mutmut_1(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(None)
    
    xǁCoverWǁhas_correlation_id__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCoverWǁhas_correlation_id__mutmut_1': xǁCoverWǁhas_correlation_id__mutmut_1
    }
    
    def has_correlation_id(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCoverWǁhas_correlation_id__mutmut_orig"), object.__getattribute__(self, "xǁCoverWǁhas_correlation_id__mutmut_mutants"), args, kwargs, self)
        return result 
    
    has_correlation_id.__signature__ = _mutmut_signature(xǁCoverWǁhas_correlation_id__mutmut_orig)
    xǁCoverWǁhas_correlation_id__mutmut_orig.__name__ = 'xǁCoverWǁhas_correlation_id'

    def xǁCoverWǁroot_uuid__mutmut_orig(self) -> Optional[PyUUID]:
        """Extract the root UUID."""
        if not self.proto.HasField("root"):
            return None
        try:
            return PyUUID(bytes=self.proto.root.value)
        except ValueError:
            return None

    def xǁCoverWǁroot_uuid__mutmut_1(self) -> Optional[PyUUID]:
        """Extract the root UUID."""
        if self.proto.HasField("root"):
            return None
        try:
            return PyUUID(bytes=self.proto.root.value)
        except ValueError:
            return None

    def xǁCoverWǁroot_uuid__mutmut_2(self) -> Optional[PyUUID]:
        """Extract the root UUID."""
        if not self.proto.HasField(None):
            return None
        try:
            return PyUUID(bytes=self.proto.root.value)
        except ValueError:
            return None

    def xǁCoverWǁroot_uuid__mutmut_3(self) -> Optional[PyUUID]:
        """Extract the root UUID."""
        if not self.proto.HasField("XXrootXX"):
            return None
        try:
            return PyUUID(bytes=self.proto.root.value)
        except ValueError:
            return None

    def xǁCoverWǁroot_uuid__mutmut_4(self) -> Optional[PyUUID]:
        """Extract the root UUID."""
        if not self.proto.HasField("ROOT"):
            return None
        try:
            return PyUUID(bytes=self.proto.root.value)
        except ValueError:
            return None

    def xǁCoverWǁroot_uuid__mutmut_5(self) -> Optional[PyUUID]:
        """Extract the root UUID."""
        if not self.proto.HasField("root"):
            return None
        try:
            return PyUUID(bytes=None)
        except ValueError:
            return None
    
    xǁCoverWǁroot_uuid__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCoverWǁroot_uuid__mutmut_1': xǁCoverWǁroot_uuid__mutmut_1, 
        'xǁCoverWǁroot_uuid__mutmut_2': xǁCoverWǁroot_uuid__mutmut_2, 
        'xǁCoverWǁroot_uuid__mutmut_3': xǁCoverWǁroot_uuid__mutmut_3, 
        'xǁCoverWǁroot_uuid__mutmut_4': xǁCoverWǁroot_uuid__mutmut_4, 
        'xǁCoverWǁroot_uuid__mutmut_5': xǁCoverWǁroot_uuid__mutmut_5
    }
    
    def root_uuid(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCoverWǁroot_uuid__mutmut_orig"), object.__getattribute__(self, "xǁCoverWǁroot_uuid__mutmut_mutants"), args, kwargs, self)
        return result 
    
    root_uuid.__signature__ = _mutmut_signature(xǁCoverWǁroot_uuid__mutmut_orig)
    xǁCoverWǁroot_uuid__mutmut_orig.__name__ = 'xǁCoverWǁroot_uuid'

    def xǁCoverWǁroot_id_hex__mutmut_orig(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        if not self.proto.HasField("root"):
            return ""
        return self.proto.root.value.hex()

    def xǁCoverWǁroot_id_hex__mutmut_1(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        if self.proto.HasField("root"):
            return ""
        return self.proto.root.value.hex()

    def xǁCoverWǁroot_id_hex__mutmut_2(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        if not self.proto.HasField(None):
            return ""
        return self.proto.root.value.hex()

    def xǁCoverWǁroot_id_hex__mutmut_3(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        if not self.proto.HasField("XXrootXX"):
            return ""
        return self.proto.root.value.hex()

    def xǁCoverWǁroot_id_hex__mutmut_4(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        if not self.proto.HasField("ROOT"):
            return ""
        return self.proto.root.value.hex()

    def xǁCoverWǁroot_id_hex__mutmut_5(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        if not self.proto.HasField("root"):
            return "XXXX"
        return self.proto.root.value.hex()
    
    xǁCoverWǁroot_id_hex__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCoverWǁroot_id_hex__mutmut_1': xǁCoverWǁroot_id_hex__mutmut_1, 
        'xǁCoverWǁroot_id_hex__mutmut_2': xǁCoverWǁroot_id_hex__mutmut_2, 
        'xǁCoverWǁroot_id_hex__mutmut_3': xǁCoverWǁroot_id_hex__mutmut_3, 
        'xǁCoverWǁroot_id_hex__mutmut_4': xǁCoverWǁroot_id_hex__mutmut_4, 
        'xǁCoverWǁroot_id_hex__mutmut_5': xǁCoverWǁroot_id_hex__mutmut_5
    }
    
    def root_id_hex(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCoverWǁroot_id_hex__mutmut_orig"), object.__getattribute__(self, "xǁCoverWǁroot_id_hex__mutmut_mutants"), args, kwargs, self)
        return result 
    
    root_id_hex.__signature__ = _mutmut_signature(xǁCoverWǁroot_id_hex__mutmut_orig)
    xǁCoverWǁroot_id_hex__mutmut_orig.__name__ = 'xǁCoverWǁroot_id_hex'

    def xǁCoverWǁedition__mutmut_orig(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        if not self.proto.HasField("edition") or not self.proto.edition.name:
            return DEFAULT_EDITION
        return self.proto.edition.name

    def xǁCoverWǁedition__mutmut_1(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        if not self.proto.HasField("edition") and not self.proto.edition.name:
            return DEFAULT_EDITION
        return self.proto.edition.name

    def xǁCoverWǁedition__mutmut_2(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        if self.proto.HasField("edition") or not self.proto.edition.name:
            return DEFAULT_EDITION
        return self.proto.edition.name

    def xǁCoverWǁedition__mutmut_3(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        if not self.proto.HasField(None) or not self.proto.edition.name:
            return DEFAULT_EDITION
        return self.proto.edition.name

    def xǁCoverWǁedition__mutmut_4(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        if not self.proto.HasField("XXeditionXX") or not self.proto.edition.name:
            return DEFAULT_EDITION
        return self.proto.edition.name

    def xǁCoverWǁedition__mutmut_5(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        if not self.proto.HasField("EDITION") or not self.proto.edition.name:
            return DEFAULT_EDITION
        return self.proto.edition.name

    def xǁCoverWǁedition__mutmut_6(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        if not self.proto.HasField("edition") or self.proto.edition.name:
            return DEFAULT_EDITION
        return self.proto.edition.name
    
    xǁCoverWǁedition__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCoverWǁedition__mutmut_1': xǁCoverWǁedition__mutmut_1, 
        'xǁCoverWǁedition__mutmut_2': xǁCoverWǁedition__mutmut_2, 
        'xǁCoverWǁedition__mutmut_3': xǁCoverWǁedition__mutmut_3, 
        'xǁCoverWǁedition__mutmut_4': xǁCoverWǁedition__mutmut_4, 
        'xǁCoverWǁedition__mutmut_5': xǁCoverWǁedition__mutmut_5, 
        'xǁCoverWǁedition__mutmut_6': xǁCoverWǁedition__mutmut_6
    }
    
    def edition(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCoverWǁedition__mutmut_orig"), object.__getattribute__(self, "xǁCoverWǁedition__mutmut_mutants"), args, kwargs, self)
        return result 
    
    edition.__signature__ = _mutmut_signature(xǁCoverWǁedition__mutmut_orig)
    xǁCoverWǁedition__mutmut_orig.__name__ = 'xǁCoverWǁedition'

    def xǁCoverWǁedition_opt__mutmut_orig(self) -> Optional[str]:
        """Return the edition name as Optional, None if not set."""
        if not self.proto.HasField("edition") or not self.proto.edition.name:
            return None
        return self.proto.edition.name

    def xǁCoverWǁedition_opt__mutmut_1(self) -> Optional[str]:
        """Return the edition name as Optional, None if not set."""
        if not self.proto.HasField("edition") and not self.proto.edition.name:
            return None
        return self.proto.edition.name

    def xǁCoverWǁedition_opt__mutmut_2(self) -> Optional[str]:
        """Return the edition name as Optional, None if not set."""
        if self.proto.HasField("edition") or not self.proto.edition.name:
            return None
        return self.proto.edition.name

    def xǁCoverWǁedition_opt__mutmut_3(self) -> Optional[str]:
        """Return the edition name as Optional, None if not set."""
        if not self.proto.HasField(None) or not self.proto.edition.name:
            return None
        return self.proto.edition.name

    def xǁCoverWǁedition_opt__mutmut_4(self) -> Optional[str]:
        """Return the edition name as Optional, None if not set."""
        if not self.proto.HasField("XXeditionXX") or not self.proto.edition.name:
            return None
        return self.proto.edition.name

    def xǁCoverWǁedition_opt__mutmut_5(self) -> Optional[str]:
        """Return the edition name as Optional, None if not set."""
        if not self.proto.HasField("EDITION") or not self.proto.edition.name:
            return None
        return self.proto.edition.name

    def xǁCoverWǁedition_opt__mutmut_6(self) -> Optional[str]:
        """Return the edition name as Optional, None if not set."""
        if not self.proto.HasField("edition") or self.proto.edition.name:
            return None
        return self.proto.edition.name
    
    xǁCoverWǁedition_opt__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCoverWǁedition_opt__mutmut_1': xǁCoverWǁedition_opt__mutmut_1, 
        'xǁCoverWǁedition_opt__mutmut_2': xǁCoverWǁedition_opt__mutmut_2, 
        'xǁCoverWǁedition_opt__mutmut_3': xǁCoverWǁedition_opt__mutmut_3, 
        'xǁCoverWǁedition_opt__mutmut_4': xǁCoverWǁedition_opt__mutmut_4, 
        'xǁCoverWǁedition_opt__mutmut_5': xǁCoverWǁedition_opt__mutmut_5, 
        'xǁCoverWǁedition_opt__mutmut_6': xǁCoverWǁedition_opt__mutmut_6
    }
    
    def edition_opt(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCoverWǁedition_opt__mutmut_orig"), object.__getattribute__(self, "xǁCoverWǁedition_opt__mutmut_mutants"), args, kwargs, self)
        return result 
    
    edition_opt.__signature__ = _mutmut_signature(xǁCoverWǁedition_opt__mutmut_orig)
    xǁCoverWǁedition_opt__mutmut_orig.__name__ = 'xǁCoverWǁedition_opt'

    def routing_key(self) -> str:
        """Compute the bus routing key."""
        return self.domain()

    def cache_key(self) -> str:
        """Generate a cache key based on domain + root."""
        return f"{self.domain()}:{self.root_id_hex()}"


class EventBookW:
    """Wrapper for EventBook proto with extension methods."""

    def xǁEventBookWǁ__init____mutmut_orig(self, proto: EventBook) -> None:
        self.proto = proto

    def xǁEventBookWǁ__init____mutmut_1(self, proto: EventBook) -> None:
        self.proto = None
    
    xǁEventBookWǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁ__init____mutmut_1': xǁEventBookWǁ__init____mutmut_1
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁEventBookWǁ__init____mutmut_orig)
    xǁEventBookWǁ__init____mutmut_orig.__name__ = 'xǁEventBookWǁ__init__'

    def next_sequence(self) -> int:
        """Return the next sequence number."""
        return self.proto.next_sequence

    def xǁEventBookWǁpages__mutmut_orig(self) -> List["EventPageW"]:
        """Return the event pages as wrapped EventPageW instances."""
        return [EventPageW(p) for p in self.proto.pages]

    def xǁEventBookWǁpages__mutmut_1(self) -> List["EventPageW"]:
        """Return the event pages as wrapped EventPageW instances."""
        return [EventPageW(None) for p in self.proto.pages]
    
    xǁEventBookWǁpages__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁpages__mutmut_1': xǁEventBookWǁpages__mutmut_1
    }
    
    def pages(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁpages__mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁpages__mutmut_mutants"), args, kwargs, self)
        return result 
    
    pages.__signature__ = _mutmut_signature(xǁEventBookWǁpages__mutmut_orig)
    xǁEventBookWǁpages__mutmut_orig.__name__ = 'xǁEventBookWǁpages'

    def xǁEventBookWǁ_cover__mutmut_orig(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("cover"):
            return None
        return self.proto.cover

    def xǁEventBookWǁ_cover__mutmut_1(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if self.proto.HasField("cover"):
            return None
        return self.proto.cover

    def xǁEventBookWǁ_cover__mutmut_2(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField(None):
            return None
        return self.proto.cover

    def xǁEventBookWǁ_cover__mutmut_3(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("XXcoverXX"):
            return None
        return self.proto.cover

    def xǁEventBookWǁ_cover__mutmut_4(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("COVER"):
            return None
        return self.proto.cover
    
    xǁEventBookWǁ_cover__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁ_cover__mutmut_1': xǁEventBookWǁ_cover__mutmut_1, 
        'xǁEventBookWǁ_cover__mutmut_2': xǁEventBookWǁ_cover__mutmut_2, 
        'xǁEventBookWǁ_cover__mutmut_3': xǁEventBookWǁ_cover__mutmut_3, 
        'xǁEventBookWǁ_cover__mutmut_4': xǁEventBookWǁ_cover__mutmut_4
    }
    
    def _cover(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁ_cover__mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁ_cover__mutmut_mutants"), args, kwargs, self)
        return result 
    
    _cover.__signature__ = _mutmut_signature(xǁEventBookWǁ_cover__mutmut_orig)
    xǁEventBookWǁ_cover__mutmut_orig.__name__ = 'xǁEventBookWǁ_cover'

    def xǁEventBookWǁdomain__mutmut_orig(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁEventBookWǁdomain__mutmut_1(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = None
        if cover is None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁEventBookWǁdomain__mutmut_2(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None and not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁEventBookWǁdomain__mutmut_3(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is not None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁEventBookWǁdomain__mutmut_4(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None or cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain
    
    xǁEventBookWǁdomain__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁdomain__mutmut_1': xǁEventBookWǁdomain__mutmut_1, 
        'xǁEventBookWǁdomain__mutmut_2': xǁEventBookWǁdomain__mutmut_2, 
        'xǁEventBookWǁdomain__mutmut_3': xǁEventBookWǁdomain__mutmut_3, 
        'xǁEventBookWǁdomain__mutmut_4': xǁEventBookWǁdomain__mutmut_4
    }
    
    def domain(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁdomain__mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁdomain__mutmut_mutants"), args, kwargs, self)
        return result 
    
    domain.__signature__ = _mutmut_signature(xǁEventBookWǁdomain__mutmut_orig)
    xǁEventBookWǁdomain__mutmut_orig.__name__ = 'xǁEventBookWǁdomain'

    def xǁEventBookWǁcorrelation_id__mutmut_orig(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is None:
            return ""
        return cover.correlation_id

    def xǁEventBookWǁcorrelation_id__mutmut_1(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = None
        if cover is None:
            return ""
        return cover.correlation_id

    def xǁEventBookWǁcorrelation_id__mutmut_2(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is not None:
            return ""
        return cover.correlation_id

    def xǁEventBookWǁcorrelation_id__mutmut_3(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is None:
            return "XXXX"
        return cover.correlation_id
    
    xǁEventBookWǁcorrelation_id__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁcorrelation_id__mutmut_1': xǁEventBookWǁcorrelation_id__mutmut_1, 
        'xǁEventBookWǁcorrelation_id__mutmut_2': xǁEventBookWǁcorrelation_id__mutmut_2, 
        'xǁEventBookWǁcorrelation_id__mutmut_3': xǁEventBookWǁcorrelation_id__mutmut_3
    }
    
    def correlation_id(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁcorrelation_id__mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁcorrelation_id__mutmut_mutants"), args, kwargs, self)
        return result 
    
    correlation_id.__signature__ = _mutmut_signature(xǁEventBookWǁcorrelation_id__mutmut_orig)
    xǁEventBookWǁcorrelation_id__mutmut_orig.__name__ = 'xǁEventBookWǁcorrelation_id'

    def xǁEventBookWǁhas_correlation_id__mutmut_orig(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(self.correlation_id())

    def xǁEventBookWǁhas_correlation_id__mutmut_1(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(None)
    
    xǁEventBookWǁhas_correlation_id__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁhas_correlation_id__mutmut_1': xǁEventBookWǁhas_correlation_id__mutmut_1
    }
    
    def has_correlation_id(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁhas_correlation_id__mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁhas_correlation_id__mutmut_mutants"), args, kwargs, self)
        return result 
    
    has_correlation_id.__signature__ = _mutmut_signature(xǁEventBookWǁhas_correlation_id__mutmut_orig)
    xǁEventBookWǁhas_correlation_id__mutmut_orig.__name__ = 'xǁEventBookWǁhas_correlation_id'

    def xǁEventBookWǁroot_uuid__mutmut_orig(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁEventBookWǁroot_uuid__mutmut_1(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = None
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁEventBookWǁroot_uuid__mutmut_2(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None and not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁEventBookWǁroot_uuid__mutmut_3(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is not None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁEventBookWǁroot_uuid__mutmut_4(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁEventBookWǁroot_uuid__mutmut_5(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField(None):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁEventBookWǁroot_uuid__mutmut_6(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("XXrootXX"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁEventBookWǁroot_uuid__mutmut_7(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("ROOT"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁEventBookWǁroot_uuid__mutmut_8(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=None)
        except ValueError:
            return None
    
    xǁEventBookWǁroot_uuid__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁroot_uuid__mutmut_1': xǁEventBookWǁroot_uuid__mutmut_1, 
        'xǁEventBookWǁroot_uuid__mutmut_2': xǁEventBookWǁroot_uuid__mutmut_2, 
        'xǁEventBookWǁroot_uuid__mutmut_3': xǁEventBookWǁroot_uuid__mutmut_3, 
        'xǁEventBookWǁroot_uuid__mutmut_4': xǁEventBookWǁroot_uuid__mutmut_4, 
        'xǁEventBookWǁroot_uuid__mutmut_5': xǁEventBookWǁroot_uuid__mutmut_5, 
        'xǁEventBookWǁroot_uuid__mutmut_6': xǁEventBookWǁroot_uuid__mutmut_6, 
        'xǁEventBookWǁroot_uuid__mutmut_7': xǁEventBookWǁroot_uuid__mutmut_7, 
        'xǁEventBookWǁroot_uuid__mutmut_8': xǁEventBookWǁroot_uuid__mutmut_8
    }
    
    def root_uuid(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁroot_uuid__mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁroot_uuid__mutmut_mutants"), args, kwargs, self)
        return result 
    
    root_uuid.__signature__ = _mutmut_signature(xǁEventBookWǁroot_uuid__mutmut_orig)
    xǁEventBookWǁroot_uuid__mutmut_orig.__name__ = 'xǁEventBookWǁroot_uuid'

    def xǁEventBookWǁroot_id_hex__mutmut_orig(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return ""
        return cover.root.value.hex()

    def xǁEventBookWǁroot_id_hex__mutmut_1(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = None
        if cover is None or not cover.HasField("root"):
            return ""
        return cover.root.value.hex()

    def xǁEventBookWǁroot_id_hex__mutmut_2(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = self._cover()
        if cover is None and not cover.HasField("root"):
            return ""
        return cover.root.value.hex()

    def xǁEventBookWǁroot_id_hex__mutmut_3(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = self._cover()
        if cover is not None or not cover.HasField("root"):
            return ""
        return cover.root.value.hex()

    def xǁEventBookWǁroot_id_hex__mutmut_4(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = self._cover()
        if cover is None or cover.HasField("root"):
            return ""
        return cover.root.value.hex()

    def xǁEventBookWǁroot_id_hex__mutmut_5(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = self._cover()
        if cover is None or not cover.HasField(None):
            return ""
        return cover.root.value.hex()

    def xǁEventBookWǁroot_id_hex__mutmut_6(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = self._cover()
        if cover is None or not cover.HasField("XXrootXX"):
            return ""
        return cover.root.value.hex()

    def xǁEventBookWǁroot_id_hex__mutmut_7(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = self._cover()
        if cover is None or not cover.HasField("ROOT"):
            return ""
        return cover.root.value.hex()

    def xǁEventBookWǁroot_id_hex__mutmut_8(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return "XXXX"
        return cover.root.value.hex()
    
    xǁEventBookWǁroot_id_hex__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁroot_id_hex__mutmut_1': xǁEventBookWǁroot_id_hex__mutmut_1, 
        'xǁEventBookWǁroot_id_hex__mutmut_2': xǁEventBookWǁroot_id_hex__mutmut_2, 
        'xǁEventBookWǁroot_id_hex__mutmut_3': xǁEventBookWǁroot_id_hex__mutmut_3, 
        'xǁEventBookWǁroot_id_hex__mutmut_4': xǁEventBookWǁroot_id_hex__mutmut_4, 
        'xǁEventBookWǁroot_id_hex__mutmut_5': xǁEventBookWǁroot_id_hex__mutmut_5, 
        'xǁEventBookWǁroot_id_hex__mutmut_6': xǁEventBookWǁroot_id_hex__mutmut_6, 
        'xǁEventBookWǁroot_id_hex__mutmut_7': xǁEventBookWǁroot_id_hex__mutmut_7, 
        'xǁEventBookWǁroot_id_hex__mutmut_8': xǁEventBookWǁroot_id_hex__mutmut_8
    }
    
    def root_id_hex(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁroot_id_hex__mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁroot_id_hex__mutmut_mutants"), args, kwargs, self)
        return result 
    
    root_id_hex.__signature__ = _mutmut_signature(xǁEventBookWǁroot_id_hex__mutmut_orig)
    xǁEventBookWǁroot_id_hex__mutmut_orig.__name__ = 'xǁEventBookWǁroot_id_hex'

    def xǁEventBookWǁedition__mutmut_orig(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is None or not cover.HasField("edition") or not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def xǁEventBookWǁedition__mutmut_1(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = None
        if cover is None or not cover.HasField("edition") or not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def xǁEventBookWǁedition__mutmut_2(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is None or not cover.HasField("edition") and not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def xǁEventBookWǁedition__mutmut_3(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is None and not cover.HasField("edition") or not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def xǁEventBookWǁedition__mutmut_4(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is not None or not cover.HasField("edition") or not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def xǁEventBookWǁedition__mutmut_5(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is None or cover.HasField("edition") or not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def xǁEventBookWǁedition__mutmut_6(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is None or not cover.HasField(None) or not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def xǁEventBookWǁedition__mutmut_7(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is None or not cover.HasField("XXeditionXX") or not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def xǁEventBookWǁedition__mutmut_8(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is None or not cover.HasField("EDITION") or not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def xǁEventBookWǁedition__mutmut_9(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is None or not cover.HasField("edition") or cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name
    
    xǁEventBookWǁedition__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁedition__mutmut_1': xǁEventBookWǁedition__mutmut_1, 
        'xǁEventBookWǁedition__mutmut_2': xǁEventBookWǁedition__mutmut_2, 
        'xǁEventBookWǁedition__mutmut_3': xǁEventBookWǁedition__mutmut_3, 
        'xǁEventBookWǁedition__mutmut_4': xǁEventBookWǁedition__mutmut_4, 
        'xǁEventBookWǁedition__mutmut_5': xǁEventBookWǁedition__mutmut_5, 
        'xǁEventBookWǁedition__mutmut_6': xǁEventBookWǁedition__mutmut_6, 
        'xǁEventBookWǁedition__mutmut_7': xǁEventBookWǁedition__mutmut_7, 
        'xǁEventBookWǁedition__mutmut_8': xǁEventBookWǁedition__mutmut_8, 
        'xǁEventBookWǁedition__mutmut_9': xǁEventBookWǁedition__mutmut_9
    }
    
    def edition(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁedition__mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁedition__mutmut_mutants"), args, kwargs, self)
        return result 
    
    edition.__signature__ = _mutmut_signature(xǁEventBookWǁedition__mutmut_orig)
    xǁEventBookWǁedition__mutmut_orig.__name__ = 'xǁEventBookWǁedition'

    def routing_key(self) -> str:
        """Compute the bus routing key."""
        return self.domain()

    def cache_key(self) -> str:
        """Generate a cache key based on domain + root."""
        return f"{self.domain()}:{self.root_id_hex()}"

    def xǁEventBookWǁcover_wrapper__mutmut_orig(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(Cover())
        return CoverW(cover)

    def xǁEventBookWǁcover_wrapper__mutmut_1(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = None
        if cover is None:
            return CoverW(Cover())
        return CoverW(cover)

    def xǁEventBookWǁcover_wrapper__mutmut_2(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is not None:
            return CoverW(Cover())
        return CoverW(cover)

    def xǁEventBookWǁcover_wrapper__mutmut_3(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(None)
        return CoverW(cover)

    def xǁEventBookWǁcover_wrapper__mutmut_4(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(Cover())
        return CoverW(None)
    
    xǁEventBookWǁcover_wrapper__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventBookWǁcover_wrapper__mutmut_1': xǁEventBookWǁcover_wrapper__mutmut_1, 
        'xǁEventBookWǁcover_wrapper__mutmut_2': xǁEventBookWǁcover_wrapper__mutmut_2, 
        'xǁEventBookWǁcover_wrapper__mutmut_3': xǁEventBookWǁcover_wrapper__mutmut_3, 
        'xǁEventBookWǁcover_wrapper__mutmut_4': xǁEventBookWǁcover_wrapper__mutmut_4
    }
    
    def cover_wrapper(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventBookWǁcover_wrapper__mutmut_orig"), object.__getattribute__(self, "xǁEventBookWǁcover_wrapper__mutmut_mutants"), args, kwargs, self)
        return result 
    
    cover_wrapper.__signature__ = _mutmut_signature(xǁEventBookWǁcover_wrapper__mutmut_orig)
    xǁEventBookWǁcover_wrapper__mutmut_orig.__name__ = 'xǁEventBookWǁcover_wrapper'


class CommandBookW:
    """Wrapper for CommandBook proto with extension methods."""

    def xǁCommandBookWǁ__init____mutmut_orig(self, proto: CommandBook) -> None:
        self.proto = proto

    def xǁCommandBookWǁ__init____mutmut_1(self, proto: CommandBook) -> None:
        self.proto = None
    
    xǁCommandBookWǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandBookWǁ__init____mutmut_1': xǁCommandBookWǁ__init____mutmut_1
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandBookWǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁCommandBookWǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁCommandBookWǁ__init____mutmut_orig)
    xǁCommandBookWǁ__init____mutmut_orig.__name__ = 'xǁCommandBookWǁ__init__'

    def xǁCommandBookWǁpages__mutmut_orig(self) -> List["CommandPageW"]:
        """Return the command pages as wrapped CommandPageW instances."""
        return [CommandPageW(p) for p in self.proto.pages]

    def xǁCommandBookWǁpages__mutmut_1(self) -> List["CommandPageW"]:
        """Return the command pages as wrapped CommandPageW instances."""
        return [CommandPageW(None) for p in self.proto.pages]
    
    xǁCommandBookWǁpages__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandBookWǁpages__mutmut_1': xǁCommandBookWǁpages__mutmut_1
    }
    
    def pages(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandBookWǁpages__mutmut_orig"), object.__getattribute__(self, "xǁCommandBookWǁpages__mutmut_mutants"), args, kwargs, self)
        return result 
    
    pages.__signature__ = _mutmut_signature(xǁCommandBookWǁpages__mutmut_orig)
    xǁCommandBookWǁpages__mutmut_orig.__name__ = 'xǁCommandBookWǁpages'

    def xǁCommandBookWǁ_cover__mutmut_orig(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("cover"):
            return None
        return self.proto.cover

    def xǁCommandBookWǁ_cover__mutmut_1(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if self.proto.HasField("cover"):
            return None
        return self.proto.cover

    def xǁCommandBookWǁ_cover__mutmut_2(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField(None):
            return None
        return self.proto.cover

    def xǁCommandBookWǁ_cover__mutmut_3(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("XXcoverXX"):
            return None
        return self.proto.cover

    def xǁCommandBookWǁ_cover__mutmut_4(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("COVER"):
            return None
        return self.proto.cover
    
    xǁCommandBookWǁ_cover__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandBookWǁ_cover__mutmut_1': xǁCommandBookWǁ_cover__mutmut_1, 
        'xǁCommandBookWǁ_cover__mutmut_2': xǁCommandBookWǁ_cover__mutmut_2, 
        'xǁCommandBookWǁ_cover__mutmut_3': xǁCommandBookWǁ_cover__mutmut_3, 
        'xǁCommandBookWǁ_cover__mutmut_4': xǁCommandBookWǁ_cover__mutmut_4
    }
    
    def _cover(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandBookWǁ_cover__mutmut_orig"), object.__getattribute__(self, "xǁCommandBookWǁ_cover__mutmut_mutants"), args, kwargs, self)
        return result 
    
    _cover.__signature__ = _mutmut_signature(xǁCommandBookWǁ_cover__mutmut_orig)
    xǁCommandBookWǁ_cover__mutmut_orig.__name__ = 'xǁCommandBookWǁ_cover'

    def xǁCommandBookWǁdomain__mutmut_orig(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁCommandBookWǁdomain__mutmut_1(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = None
        if cover is None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁCommandBookWǁdomain__mutmut_2(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None and not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁCommandBookWǁdomain__mutmut_3(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is not None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁCommandBookWǁdomain__mutmut_4(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None or cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain
    
    xǁCommandBookWǁdomain__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandBookWǁdomain__mutmut_1': xǁCommandBookWǁdomain__mutmut_1, 
        'xǁCommandBookWǁdomain__mutmut_2': xǁCommandBookWǁdomain__mutmut_2, 
        'xǁCommandBookWǁdomain__mutmut_3': xǁCommandBookWǁdomain__mutmut_3, 
        'xǁCommandBookWǁdomain__mutmut_4': xǁCommandBookWǁdomain__mutmut_4
    }
    
    def domain(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandBookWǁdomain__mutmut_orig"), object.__getattribute__(self, "xǁCommandBookWǁdomain__mutmut_mutants"), args, kwargs, self)
        return result 
    
    domain.__signature__ = _mutmut_signature(xǁCommandBookWǁdomain__mutmut_orig)
    xǁCommandBookWǁdomain__mutmut_orig.__name__ = 'xǁCommandBookWǁdomain'

    def xǁCommandBookWǁcorrelation_id__mutmut_orig(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is None:
            return ""
        return cover.correlation_id

    def xǁCommandBookWǁcorrelation_id__mutmut_1(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = None
        if cover is None:
            return ""
        return cover.correlation_id

    def xǁCommandBookWǁcorrelation_id__mutmut_2(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is not None:
            return ""
        return cover.correlation_id

    def xǁCommandBookWǁcorrelation_id__mutmut_3(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is None:
            return "XXXX"
        return cover.correlation_id
    
    xǁCommandBookWǁcorrelation_id__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandBookWǁcorrelation_id__mutmut_1': xǁCommandBookWǁcorrelation_id__mutmut_1, 
        'xǁCommandBookWǁcorrelation_id__mutmut_2': xǁCommandBookWǁcorrelation_id__mutmut_2, 
        'xǁCommandBookWǁcorrelation_id__mutmut_3': xǁCommandBookWǁcorrelation_id__mutmut_3
    }
    
    def correlation_id(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandBookWǁcorrelation_id__mutmut_orig"), object.__getattribute__(self, "xǁCommandBookWǁcorrelation_id__mutmut_mutants"), args, kwargs, self)
        return result 
    
    correlation_id.__signature__ = _mutmut_signature(xǁCommandBookWǁcorrelation_id__mutmut_orig)
    xǁCommandBookWǁcorrelation_id__mutmut_orig.__name__ = 'xǁCommandBookWǁcorrelation_id'

    def xǁCommandBookWǁhas_correlation_id__mutmut_orig(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(self.correlation_id())

    def xǁCommandBookWǁhas_correlation_id__mutmut_1(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(None)
    
    xǁCommandBookWǁhas_correlation_id__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandBookWǁhas_correlation_id__mutmut_1': xǁCommandBookWǁhas_correlation_id__mutmut_1
    }
    
    def has_correlation_id(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandBookWǁhas_correlation_id__mutmut_orig"), object.__getattribute__(self, "xǁCommandBookWǁhas_correlation_id__mutmut_mutants"), args, kwargs, self)
        return result 
    
    has_correlation_id.__signature__ = _mutmut_signature(xǁCommandBookWǁhas_correlation_id__mutmut_orig)
    xǁCommandBookWǁhas_correlation_id__mutmut_orig.__name__ = 'xǁCommandBookWǁhas_correlation_id'

    def xǁCommandBookWǁroot_uuid__mutmut_orig(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁCommandBookWǁroot_uuid__mutmut_1(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = None
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁCommandBookWǁroot_uuid__mutmut_2(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None and not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁCommandBookWǁroot_uuid__mutmut_3(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is not None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁCommandBookWǁroot_uuid__mutmut_4(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁCommandBookWǁroot_uuid__mutmut_5(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField(None):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁCommandBookWǁroot_uuid__mutmut_6(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("XXrootXX"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁCommandBookWǁroot_uuid__mutmut_7(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("ROOT"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁCommandBookWǁroot_uuid__mutmut_8(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=None)
        except ValueError:
            return None
    
    xǁCommandBookWǁroot_uuid__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandBookWǁroot_uuid__mutmut_1': xǁCommandBookWǁroot_uuid__mutmut_1, 
        'xǁCommandBookWǁroot_uuid__mutmut_2': xǁCommandBookWǁroot_uuid__mutmut_2, 
        'xǁCommandBookWǁroot_uuid__mutmut_3': xǁCommandBookWǁroot_uuid__mutmut_3, 
        'xǁCommandBookWǁroot_uuid__mutmut_4': xǁCommandBookWǁroot_uuid__mutmut_4, 
        'xǁCommandBookWǁroot_uuid__mutmut_5': xǁCommandBookWǁroot_uuid__mutmut_5, 
        'xǁCommandBookWǁroot_uuid__mutmut_6': xǁCommandBookWǁroot_uuid__mutmut_6, 
        'xǁCommandBookWǁroot_uuid__mutmut_7': xǁCommandBookWǁroot_uuid__mutmut_7, 
        'xǁCommandBookWǁroot_uuid__mutmut_8': xǁCommandBookWǁroot_uuid__mutmut_8
    }
    
    def root_uuid(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandBookWǁroot_uuid__mutmut_orig"), object.__getattribute__(self, "xǁCommandBookWǁroot_uuid__mutmut_mutants"), args, kwargs, self)
        return result 
    
    root_uuid.__signature__ = _mutmut_signature(xǁCommandBookWǁroot_uuid__mutmut_orig)
    xǁCommandBookWǁroot_uuid__mutmut_orig.__name__ = 'xǁCommandBookWǁroot_uuid'

    def routing_key(self) -> str:
        """Compute the bus routing key."""
        return self.domain()

    def xǁCommandBookWǁcache_key__mutmut_orig(self) -> str:
        """Generate a cache key based on domain + root."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return f"{self.domain()}:"
        return f"{self.domain()}:{cover.root.value.hex()}"

    def xǁCommandBookWǁcache_key__mutmut_1(self) -> str:
        """Generate a cache key based on domain + root."""
        cover = None
        if cover is None or not cover.HasField("root"):
            return f"{self.domain()}:"
        return f"{self.domain()}:{cover.root.value.hex()}"

    def xǁCommandBookWǁcache_key__mutmut_2(self) -> str:
        """Generate a cache key based on domain + root."""
        cover = self._cover()
        if cover is None and not cover.HasField("root"):
            return f"{self.domain()}:"
        return f"{self.domain()}:{cover.root.value.hex()}"

    def xǁCommandBookWǁcache_key__mutmut_3(self) -> str:
        """Generate a cache key based on domain + root."""
        cover = self._cover()
        if cover is not None or not cover.HasField("root"):
            return f"{self.domain()}:"
        return f"{self.domain()}:{cover.root.value.hex()}"

    def xǁCommandBookWǁcache_key__mutmut_4(self) -> str:
        """Generate a cache key based on domain + root."""
        cover = self._cover()
        if cover is None or cover.HasField("root"):
            return f"{self.domain()}:"
        return f"{self.domain()}:{cover.root.value.hex()}"

    def xǁCommandBookWǁcache_key__mutmut_5(self) -> str:
        """Generate a cache key based on domain + root."""
        cover = self._cover()
        if cover is None or not cover.HasField(None):
            return f"{self.domain()}:"
        return f"{self.domain()}:{cover.root.value.hex()}"

    def xǁCommandBookWǁcache_key__mutmut_6(self) -> str:
        """Generate a cache key based on domain + root."""
        cover = self._cover()
        if cover is None or not cover.HasField("XXrootXX"):
            return f"{self.domain()}:"
        return f"{self.domain()}:{cover.root.value.hex()}"

    def xǁCommandBookWǁcache_key__mutmut_7(self) -> str:
        """Generate a cache key based on domain + root."""
        cover = self._cover()
        if cover is None or not cover.HasField("ROOT"):
            return f"{self.domain()}:"
        return f"{self.domain()}:{cover.root.value.hex()}"
    
    xǁCommandBookWǁcache_key__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandBookWǁcache_key__mutmut_1': xǁCommandBookWǁcache_key__mutmut_1, 
        'xǁCommandBookWǁcache_key__mutmut_2': xǁCommandBookWǁcache_key__mutmut_2, 
        'xǁCommandBookWǁcache_key__mutmut_3': xǁCommandBookWǁcache_key__mutmut_3, 
        'xǁCommandBookWǁcache_key__mutmut_4': xǁCommandBookWǁcache_key__mutmut_4, 
        'xǁCommandBookWǁcache_key__mutmut_5': xǁCommandBookWǁcache_key__mutmut_5, 
        'xǁCommandBookWǁcache_key__mutmut_6': xǁCommandBookWǁcache_key__mutmut_6, 
        'xǁCommandBookWǁcache_key__mutmut_7': xǁCommandBookWǁcache_key__mutmut_7
    }
    
    def cache_key(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandBookWǁcache_key__mutmut_orig"), object.__getattribute__(self, "xǁCommandBookWǁcache_key__mutmut_mutants"), args, kwargs, self)
        return result 
    
    cache_key.__signature__ = _mutmut_signature(xǁCommandBookWǁcache_key__mutmut_orig)
    xǁCommandBookWǁcache_key__mutmut_orig.__name__ = 'xǁCommandBookWǁcache_key'

    def xǁCommandBookWǁcover_wrapper__mutmut_orig(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(Cover())
        return CoverW(cover)

    def xǁCommandBookWǁcover_wrapper__mutmut_1(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = None
        if cover is None:
            return CoverW(Cover())
        return CoverW(cover)

    def xǁCommandBookWǁcover_wrapper__mutmut_2(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is not None:
            return CoverW(Cover())
        return CoverW(cover)

    def xǁCommandBookWǁcover_wrapper__mutmut_3(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(None)
        return CoverW(cover)

    def xǁCommandBookWǁcover_wrapper__mutmut_4(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(Cover())
        return CoverW(None)
    
    xǁCommandBookWǁcover_wrapper__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandBookWǁcover_wrapper__mutmut_1': xǁCommandBookWǁcover_wrapper__mutmut_1, 
        'xǁCommandBookWǁcover_wrapper__mutmut_2': xǁCommandBookWǁcover_wrapper__mutmut_2, 
        'xǁCommandBookWǁcover_wrapper__mutmut_3': xǁCommandBookWǁcover_wrapper__mutmut_3, 
        'xǁCommandBookWǁcover_wrapper__mutmut_4': xǁCommandBookWǁcover_wrapper__mutmut_4
    }
    
    def cover_wrapper(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandBookWǁcover_wrapper__mutmut_orig"), object.__getattribute__(self, "xǁCommandBookWǁcover_wrapper__mutmut_mutants"), args, kwargs, self)
        return result 
    
    cover_wrapper.__signature__ = _mutmut_signature(xǁCommandBookWǁcover_wrapper__mutmut_orig)
    xǁCommandBookWǁcover_wrapper__mutmut_orig.__name__ = 'xǁCommandBookWǁcover_wrapper'


class QueryW:
    """Wrapper for Query proto with extension methods."""

    def xǁQueryWǁ__init____mutmut_orig(self, proto: Query) -> None:
        self.proto = proto

    def xǁQueryWǁ__init____mutmut_1(self, proto: Query) -> None:
        self.proto = None
    
    xǁQueryWǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryWǁ__init____mutmut_1': xǁQueryWǁ__init____mutmut_1
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryWǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁQueryWǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁQueryWǁ__init____mutmut_orig)
    xǁQueryWǁ__init____mutmut_orig.__name__ = 'xǁQueryWǁ__init__'

    def xǁQueryWǁ_cover__mutmut_orig(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("cover"):
            return None
        return self.proto.cover

    def xǁQueryWǁ_cover__mutmut_1(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if self.proto.HasField("cover"):
            return None
        return self.proto.cover

    def xǁQueryWǁ_cover__mutmut_2(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField(None):
            return None
        return self.proto.cover

    def xǁQueryWǁ_cover__mutmut_3(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("XXcoverXX"):
            return None
        return self.proto.cover

    def xǁQueryWǁ_cover__mutmut_4(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("COVER"):
            return None
        return self.proto.cover
    
    xǁQueryWǁ_cover__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryWǁ_cover__mutmut_1': xǁQueryWǁ_cover__mutmut_1, 
        'xǁQueryWǁ_cover__mutmut_2': xǁQueryWǁ_cover__mutmut_2, 
        'xǁQueryWǁ_cover__mutmut_3': xǁQueryWǁ_cover__mutmut_3, 
        'xǁQueryWǁ_cover__mutmut_4': xǁQueryWǁ_cover__mutmut_4
    }
    
    def _cover(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryWǁ_cover__mutmut_orig"), object.__getattribute__(self, "xǁQueryWǁ_cover__mutmut_mutants"), args, kwargs, self)
        return result 
    
    _cover.__signature__ = _mutmut_signature(xǁQueryWǁ_cover__mutmut_orig)
    xǁQueryWǁ_cover__mutmut_orig.__name__ = 'xǁQueryWǁ_cover'

    def xǁQueryWǁdomain__mutmut_orig(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁQueryWǁdomain__mutmut_1(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = None
        if cover is None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁQueryWǁdomain__mutmut_2(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None and not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁQueryWǁdomain__mutmut_3(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is not None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def xǁQueryWǁdomain__mutmut_4(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None or cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain
    
    xǁQueryWǁdomain__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryWǁdomain__mutmut_1': xǁQueryWǁdomain__mutmut_1, 
        'xǁQueryWǁdomain__mutmut_2': xǁQueryWǁdomain__mutmut_2, 
        'xǁQueryWǁdomain__mutmut_3': xǁQueryWǁdomain__mutmut_3, 
        'xǁQueryWǁdomain__mutmut_4': xǁQueryWǁdomain__mutmut_4
    }
    
    def domain(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryWǁdomain__mutmut_orig"), object.__getattribute__(self, "xǁQueryWǁdomain__mutmut_mutants"), args, kwargs, self)
        return result 
    
    domain.__signature__ = _mutmut_signature(xǁQueryWǁdomain__mutmut_orig)
    xǁQueryWǁdomain__mutmut_orig.__name__ = 'xǁQueryWǁdomain'

    def xǁQueryWǁcorrelation_id__mutmut_orig(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is None:
            return ""
        return cover.correlation_id

    def xǁQueryWǁcorrelation_id__mutmut_1(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = None
        if cover is None:
            return ""
        return cover.correlation_id

    def xǁQueryWǁcorrelation_id__mutmut_2(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is not None:
            return ""
        return cover.correlation_id

    def xǁQueryWǁcorrelation_id__mutmut_3(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is None:
            return "XXXX"
        return cover.correlation_id
    
    xǁQueryWǁcorrelation_id__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryWǁcorrelation_id__mutmut_1': xǁQueryWǁcorrelation_id__mutmut_1, 
        'xǁQueryWǁcorrelation_id__mutmut_2': xǁQueryWǁcorrelation_id__mutmut_2, 
        'xǁQueryWǁcorrelation_id__mutmut_3': xǁQueryWǁcorrelation_id__mutmut_3
    }
    
    def correlation_id(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryWǁcorrelation_id__mutmut_orig"), object.__getattribute__(self, "xǁQueryWǁcorrelation_id__mutmut_mutants"), args, kwargs, self)
        return result 
    
    correlation_id.__signature__ = _mutmut_signature(xǁQueryWǁcorrelation_id__mutmut_orig)
    xǁQueryWǁcorrelation_id__mutmut_orig.__name__ = 'xǁQueryWǁcorrelation_id'

    def xǁQueryWǁhas_correlation_id__mutmut_orig(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(self.correlation_id())

    def xǁQueryWǁhas_correlation_id__mutmut_1(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(None)
    
    xǁQueryWǁhas_correlation_id__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryWǁhas_correlation_id__mutmut_1': xǁQueryWǁhas_correlation_id__mutmut_1
    }
    
    def has_correlation_id(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryWǁhas_correlation_id__mutmut_orig"), object.__getattribute__(self, "xǁQueryWǁhas_correlation_id__mutmut_mutants"), args, kwargs, self)
        return result 
    
    has_correlation_id.__signature__ = _mutmut_signature(xǁQueryWǁhas_correlation_id__mutmut_orig)
    xǁQueryWǁhas_correlation_id__mutmut_orig.__name__ = 'xǁQueryWǁhas_correlation_id'

    def xǁQueryWǁroot_uuid__mutmut_orig(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁQueryWǁroot_uuid__mutmut_1(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = None
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁQueryWǁroot_uuid__mutmut_2(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None and not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁQueryWǁroot_uuid__mutmut_3(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is not None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁQueryWǁroot_uuid__mutmut_4(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁQueryWǁroot_uuid__mutmut_5(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField(None):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁQueryWǁroot_uuid__mutmut_6(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("XXrootXX"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁQueryWǁroot_uuid__mutmut_7(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("ROOT"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def xǁQueryWǁroot_uuid__mutmut_8(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=None)
        except ValueError:
            return None
    
    xǁQueryWǁroot_uuid__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryWǁroot_uuid__mutmut_1': xǁQueryWǁroot_uuid__mutmut_1, 
        'xǁQueryWǁroot_uuid__mutmut_2': xǁQueryWǁroot_uuid__mutmut_2, 
        'xǁQueryWǁroot_uuid__mutmut_3': xǁQueryWǁroot_uuid__mutmut_3, 
        'xǁQueryWǁroot_uuid__mutmut_4': xǁQueryWǁroot_uuid__mutmut_4, 
        'xǁQueryWǁroot_uuid__mutmut_5': xǁQueryWǁroot_uuid__mutmut_5, 
        'xǁQueryWǁroot_uuid__mutmut_6': xǁQueryWǁroot_uuid__mutmut_6, 
        'xǁQueryWǁroot_uuid__mutmut_7': xǁQueryWǁroot_uuid__mutmut_7, 
        'xǁQueryWǁroot_uuid__mutmut_8': xǁQueryWǁroot_uuid__mutmut_8
    }
    
    def root_uuid(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryWǁroot_uuid__mutmut_orig"), object.__getattribute__(self, "xǁQueryWǁroot_uuid__mutmut_mutants"), args, kwargs, self)
        return result 
    
    root_uuid.__signature__ = _mutmut_signature(xǁQueryWǁroot_uuid__mutmut_orig)
    xǁQueryWǁroot_uuid__mutmut_orig.__name__ = 'xǁQueryWǁroot_uuid'

    def routing_key(self) -> str:
        """Compute the bus routing key."""
        return self.domain()

    def xǁQueryWǁcover_wrapper__mutmut_orig(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(Cover())
        return CoverW(cover)

    def xǁQueryWǁcover_wrapper__mutmut_1(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = None
        if cover is None:
            return CoverW(Cover())
        return CoverW(cover)

    def xǁQueryWǁcover_wrapper__mutmut_2(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is not None:
            return CoverW(Cover())
        return CoverW(cover)

    def xǁQueryWǁcover_wrapper__mutmut_3(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(None)
        return CoverW(cover)

    def xǁQueryWǁcover_wrapper__mutmut_4(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(Cover())
        return CoverW(None)
    
    xǁQueryWǁcover_wrapper__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁQueryWǁcover_wrapper__mutmut_1': xǁQueryWǁcover_wrapper__mutmut_1, 
        'xǁQueryWǁcover_wrapper__mutmut_2': xǁQueryWǁcover_wrapper__mutmut_2, 
        'xǁQueryWǁcover_wrapper__mutmut_3': xǁQueryWǁcover_wrapper__mutmut_3, 
        'xǁQueryWǁcover_wrapper__mutmut_4': xǁQueryWǁcover_wrapper__mutmut_4
    }
    
    def cover_wrapper(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁQueryWǁcover_wrapper__mutmut_orig"), object.__getattribute__(self, "xǁQueryWǁcover_wrapper__mutmut_mutants"), args, kwargs, self)
        return result 
    
    cover_wrapper.__signature__ = _mutmut_signature(xǁQueryWǁcover_wrapper__mutmut_orig)
    xǁQueryWǁcover_wrapper__mutmut_orig.__name__ = 'xǁQueryWǁcover_wrapper'


class EventPageW:
    """Wrapper for EventPage proto with extension methods."""

    def xǁEventPageWǁ__init____mutmut_orig(self, proto: EventPage) -> None:
        self.proto = proto

    def xǁEventPageWǁ__init____mutmut_1(self, proto: EventPage) -> None:
        self.proto = None
    
    xǁEventPageWǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventPageWǁ__init____mutmut_1': xǁEventPageWǁ__init____mutmut_1
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventPageWǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁEventPageWǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁEventPageWǁ__init____mutmut_orig)
    xǁEventPageWǁ__init____mutmut_orig.__name__ = 'xǁEventPageWǁ__init__'

    def xǁEventPageWǁdecode_event__mutmut_orig(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("event"):
            return None
        if not type_url_matches(self.proto.event.type_url, type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_1(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if self.proto.HasField("event"):
            return None
        if not type_url_matches(self.proto.event.type_url, type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_2(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField(None):
            return None
        if not type_url_matches(self.proto.event.type_url, type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_3(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("XXeventXX"):
            return None
        if not type_url_matches(self.proto.event.type_url, type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_4(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("EVENT"):
            return None
        if not type_url_matches(self.proto.event.type_url, type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_5(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("event"):
            return None
        if type_url_matches(self.proto.event.type_url, type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_6(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("event"):
            return None
        if not type_url_matches(None, type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_7(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("event"):
            return None
        if not type_url_matches(self.proto.event.type_url, None):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_8(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("event"):
            return None
        if not type_url_matches(type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_9(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("event"):
            return None
        if not type_url_matches(self.proto.event.type_url, ):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_10(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("event"):
            return None
        if not type_url_matches(self.proto.event.type_url, type_suffix):
            return None
        try:
            msg = None
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None

    def xǁEventPageWǁdecode_event__mutmut_11(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("event"):
            return None
        if not type_url_matches(self.proto.event.type_url, type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(None)
            return msg
        except Exception:
            return None
    
    xǁEventPageWǁdecode_event__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁEventPageWǁdecode_event__mutmut_1': xǁEventPageWǁdecode_event__mutmut_1, 
        'xǁEventPageWǁdecode_event__mutmut_2': xǁEventPageWǁdecode_event__mutmut_2, 
        'xǁEventPageWǁdecode_event__mutmut_3': xǁEventPageWǁdecode_event__mutmut_3, 
        'xǁEventPageWǁdecode_event__mutmut_4': xǁEventPageWǁdecode_event__mutmut_4, 
        'xǁEventPageWǁdecode_event__mutmut_5': xǁEventPageWǁdecode_event__mutmut_5, 
        'xǁEventPageWǁdecode_event__mutmut_6': xǁEventPageWǁdecode_event__mutmut_6, 
        'xǁEventPageWǁdecode_event__mutmut_7': xǁEventPageWǁdecode_event__mutmut_7, 
        'xǁEventPageWǁdecode_event__mutmut_8': xǁEventPageWǁdecode_event__mutmut_8, 
        'xǁEventPageWǁdecode_event__mutmut_9': xǁEventPageWǁdecode_event__mutmut_9, 
        'xǁEventPageWǁdecode_event__mutmut_10': xǁEventPageWǁdecode_event__mutmut_10, 
        'xǁEventPageWǁdecode_event__mutmut_11': xǁEventPageWǁdecode_event__mutmut_11
    }
    
    def decode_event(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁEventPageWǁdecode_event__mutmut_orig"), object.__getattribute__(self, "xǁEventPageWǁdecode_event__mutmut_mutants"), args, kwargs, self)
        return result 
    
    decode_event.__signature__ = _mutmut_signature(xǁEventPageWǁdecode_event__mutmut_orig)
    xǁEventPageWǁdecode_event__mutmut_orig.__name__ = 'xǁEventPageWǁdecode_event'


class CommandPageW:
    """Wrapper for CommandPage proto with extension methods."""

    def xǁCommandPageWǁ__init____mutmut_orig(self, proto: CommandPage) -> None:
        self.proto = proto

    def xǁCommandPageWǁ__init____mutmut_1(self, proto: CommandPage) -> None:
        self.proto = None
    
    xǁCommandPageWǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandPageWǁ__init____mutmut_1': xǁCommandPageWǁ__init____mutmut_1
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandPageWǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁCommandPageWǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁCommandPageWǁ__init____mutmut_orig)
    xǁCommandPageWǁ__init____mutmut_orig.__name__ = 'xǁCommandPageWǁ__init__'

    def sequence(self) -> int:
        """Return the sequence number."""
        return self.proto.sequence


class CommandResponseW:
    """Wrapper for CommandResponse proto with extension methods."""

    def xǁCommandResponseWǁ__init____mutmut_orig(self, proto: CommandResponse) -> None:
        self.proto = proto

    def xǁCommandResponseWǁ__init____mutmut_1(self, proto: CommandResponse) -> None:
        self.proto = None
    
    xǁCommandResponseWǁ__init____mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandResponseWǁ__init____mutmut_1': xǁCommandResponseWǁ__init____mutmut_1
    }
    
    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandResponseWǁ__init____mutmut_orig"), object.__getattribute__(self, "xǁCommandResponseWǁ__init____mutmut_mutants"), args, kwargs, self)
        return result 
    
    __init__.__signature__ = _mutmut_signature(xǁCommandResponseWǁ__init____mutmut_orig)
    xǁCommandResponseWǁ__init____mutmut_orig.__name__ = 'xǁCommandResponseWǁ__init__'

    def xǁCommandResponseWǁevents_book__mutmut_orig(self) -> Optional["EventBookW"]:
        """Return the events as a wrapped EventBookW, or None if not set."""
        if not self.proto.HasField("events"):
            return None
        return EventBookW(self.proto.events)

    def xǁCommandResponseWǁevents_book__mutmut_1(self) -> Optional["EventBookW"]:
        """Return the events as a wrapped EventBookW, or None if not set."""
        if self.proto.HasField("events"):
            return None
        return EventBookW(self.proto.events)

    def xǁCommandResponseWǁevents_book__mutmut_2(self) -> Optional["EventBookW"]:
        """Return the events as a wrapped EventBookW, or None if not set."""
        if not self.proto.HasField(None):
            return None
        return EventBookW(self.proto.events)

    def xǁCommandResponseWǁevents_book__mutmut_3(self) -> Optional["EventBookW"]:
        """Return the events as a wrapped EventBookW, or None if not set."""
        if not self.proto.HasField("XXeventsXX"):
            return None
        return EventBookW(self.proto.events)

    def xǁCommandResponseWǁevents_book__mutmut_4(self) -> Optional["EventBookW"]:
        """Return the events as a wrapped EventBookW, or None if not set."""
        if not self.proto.HasField("EVENTS"):
            return None
        return EventBookW(self.proto.events)

    def xǁCommandResponseWǁevents_book__mutmut_5(self) -> Optional["EventBookW"]:
        """Return the events as a wrapped EventBookW, or None if not set."""
        if not self.proto.HasField("events"):
            return None
        return EventBookW(None)
    
    xǁCommandResponseWǁevents_book__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandResponseWǁevents_book__mutmut_1': xǁCommandResponseWǁevents_book__mutmut_1, 
        'xǁCommandResponseWǁevents_book__mutmut_2': xǁCommandResponseWǁevents_book__mutmut_2, 
        'xǁCommandResponseWǁevents_book__mutmut_3': xǁCommandResponseWǁevents_book__mutmut_3, 
        'xǁCommandResponseWǁevents_book__mutmut_4': xǁCommandResponseWǁevents_book__mutmut_4, 
        'xǁCommandResponseWǁevents_book__mutmut_5': xǁCommandResponseWǁevents_book__mutmut_5
    }
    
    def events_book(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandResponseWǁevents_book__mutmut_orig"), object.__getattribute__(self, "xǁCommandResponseWǁevents_book__mutmut_mutants"), args, kwargs, self)
        return result 
    
    events_book.__signature__ = _mutmut_signature(xǁCommandResponseWǁevents_book__mutmut_orig)
    xǁCommandResponseWǁevents_book__mutmut_orig.__name__ = 'xǁCommandResponseWǁevents_book'

    def xǁCommandResponseWǁevents__mutmut_orig(self) -> List["EventPageW"]:
        """Extract the event pages as wrapped EventPageW instances."""
        if not self.proto.HasField("events"):
            return []
        return [EventPageW(p) for p in self.proto.events.pages]

    def xǁCommandResponseWǁevents__mutmut_1(self) -> List["EventPageW"]:
        """Extract the event pages as wrapped EventPageW instances."""
        if self.proto.HasField("events"):
            return []
        return [EventPageW(p) for p in self.proto.events.pages]

    def xǁCommandResponseWǁevents__mutmut_2(self) -> List["EventPageW"]:
        """Extract the event pages as wrapped EventPageW instances."""
        if not self.proto.HasField(None):
            return []
        return [EventPageW(p) for p in self.proto.events.pages]

    def xǁCommandResponseWǁevents__mutmut_3(self) -> List["EventPageW"]:
        """Extract the event pages as wrapped EventPageW instances."""
        if not self.proto.HasField("XXeventsXX"):
            return []
        return [EventPageW(p) for p in self.proto.events.pages]

    def xǁCommandResponseWǁevents__mutmut_4(self) -> List["EventPageW"]:
        """Extract the event pages as wrapped EventPageW instances."""
        if not self.proto.HasField("EVENTS"):
            return []
        return [EventPageW(p) for p in self.proto.events.pages]

    def xǁCommandResponseWǁevents__mutmut_5(self) -> List["EventPageW"]:
        """Extract the event pages as wrapped EventPageW instances."""
        if not self.proto.HasField("events"):
            return []
        return [EventPageW(None) for p in self.proto.events.pages]
    
    xǁCommandResponseWǁevents__mutmut_mutants : ClassVar[MutantDict] = {
    'xǁCommandResponseWǁevents__mutmut_1': xǁCommandResponseWǁevents__mutmut_1, 
        'xǁCommandResponseWǁevents__mutmut_2': xǁCommandResponseWǁevents__mutmut_2, 
        'xǁCommandResponseWǁevents__mutmut_3': xǁCommandResponseWǁevents__mutmut_3, 
        'xǁCommandResponseWǁevents__mutmut_4': xǁCommandResponseWǁevents__mutmut_4, 
        'xǁCommandResponseWǁevents__mutmut_5': xǁCommandResponseWǁevents__mutmut_5
    }
    
    def events(self, *args, **kwargs):
        result = _mutmut_trampoline(object.__getattribute__(self, "xǁCommandResponseWǁevents__mutmut_orig"), object.__getattribute__(self, "xǁCommandResponseWǁevents__mutmut_mutants"), args, kwargs, self)
        return result 
    
    events.__signature__ = _mutmut_signature(xǁCommandResponseWǁevents__mutmut_orig)
    xǁCommandResponseWǁevents__mutmut_orig.__name__ = 'xǁCommandResponseWǁevents'
