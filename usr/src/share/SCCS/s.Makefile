h63729
s 00000/00000/00000
d R 1.2 01/10/20 13:35:40 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/share/Makefile
c Name history : 1 0 src/share/Makefile
e
s 00026/00000/00000
d D 1.1 01/10/20 13:35:39 bmc 1 0
c date and time created 01/10/20 13:35:39 by bmc
e
u
U
f e 0
t
T
I 1
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
E 1
