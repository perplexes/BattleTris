#
#     NAME: Makefile
#   AUTHOR: Michael Shapiro
#     DATE: 11Dec94
#    DESCR: Makefile for BattleTris shared files
# MODIFIED: 13Apr95
#

include ../Makeinclude

all:	install

RELDEFAULTS	= $(RELDEFDIR)/BattleTris
RELWEAPONSD	= $(RELSHRDIR)/btweapons.db
RELWEAPONSP	= $(RELSHRDIR)/btweaponsp.db

$(RELDEFAULTS): BattleTris.ad
	$(INSTALL) -m 0444 BattleTris.ad $@

$(RELWEAPONSD):	btweapons.db
	$(INSTALL) -m 0444 btweapons.db $@

$(RELWEAPONSP):	btweaponsp.db
	$(INSTALL) -m 0444 btweaponsp.db $@

install: $(RELDEFAULTS) $(RELWEAPONSD) $(RELWEAPONSP)
