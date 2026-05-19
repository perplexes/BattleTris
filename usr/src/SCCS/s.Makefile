h61271
s 00030/00000/00066
d D 1.3 01/10/20 18:26:21 ahl 4 3
c 1000006 Makefile should have a rule for cscope stuff
e
s 00001/00001/00065
d D 1.2 01/10/20 16:24:30 ahl 3 1
c 1000005 install-sh isn't executable so needs /bin/sh in front of it
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:41 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/Makefile
c Name history : 1 0 src/Makefile
e
s 00066/00000/00000
d D 1.1 01/10/20 13:35:40 bmc 1 0
c date and time created 01/10/20 13:35:40 by bmc
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
#     DATE: 14Oct94
#    DESCR: Makefile for BattleTris source tree
# MODIFIED: 4Nov95
#

include ./Makeinclude

all: config src

config: ../include/BTConfig.H

../include/BTConfig.H: BTConfig.H
	@echo "+++ installing BattleTris configuration file"
D 3
	../bin/install-sh -c -m 0444 BTConfig.H ../include/BTConfig.H
E 3
I 3
	sh ../bin/install-sh -c -m 0444 BTConfig.H ../include/BTConfig.H
E 3

src:
	@echo "+++ building BattleTris include directory"
	@( cd game && $(MAKE) common )
	@echo "+++ building BattleTris standard library"
	@( cd stdlib && $(MAKE) install )
	@echo "+++ building BattleTris signals library"
	@( cd signals && $(MAKE) install )
	@echo "+++ building BattleTris audio library"
	@( cd audio && $(MAKE) install )
	@echo "+++ building BattleTris sockets library"
	@( cd sockets && $(MAKE) install )
	@echo "+++ building BattleTris database library"
	@( cd db && $(MAKE) install )
	@echo "+++ building BattleTris widget library"
	@( cd widget && $(MAKE) install )
	@echo "+++ building BattleTris daemons"
	@( cd daemons && $(MAKE) )
	@echo "+++ building BattleTris referee"
	@( cd btref && $(MAKE) )
	@echo "+++ building BattleTris client"
	@( cd game && $(MAKE) )

install:
	#@echo "+++ installing BattleTris documentation"
	#@( cd man && $(MAKE) install )
	@echo "+++ installing BattleTris shared files"
	@( cd share && $(MAKE) install )
	@echo "+++ installing BattleTris daemons"
	@( cd daemons && $(MAKE) install )
	@echo "+++ installing BattleTris referee"
	@( cd btref && $(MAKE) install )
	@echo "+++ installing BattleTris client"
	@( cd game && $(MAKE) install )

clean:
	@( cd game && $(MAKE) clean )
	@( cd stdlib && $(MAKE) clean )
	@( cd audio && $(MAKE) clean )
	@( cd db && $(MAKE) clean )
	@( cd signals && $(MAKE) clean )
	@( cd sockets && $(MAKE) clean )
	@( cd widget && $(MAKE) clean )
	@( cd daemons && $(MAKE) clean )
	@( cd btref && $(MAKE) clean )
	@( cd game && $(MAKE) clean )

distclean:
	$(RM) config.status config.log config.cache Makeinclude BTConfig.H
I 4

CSDIR   = .
CSDIRS  = btref daemons db game share signals sockets stdlib widget
CSPATHS = $(CSDIRS:%=$(CSDIR)/%)
CSINCS  = $(CSPATHS:%=-I%)
CSCOPE  = cscope

#
# Set CSFLAGS env variable to -bq when using fast cscope to
# build the fast (but large) cscope data bases.
#
CSFLAGS = -b

.PRECIOUS:      cscope.out

cscope.out: cscope.files
	${CSCOPE} ${CSFLAGS}

cscope.files:
	@-$(RM) cscope.files cscope.files.raw
	echo "$(CSINCS)" > cscope.files
	-find $(CSPATHS) -name SCCS -prune -o \
	    -type d -name '.del-*' -prune -o -type f \
	    \( -name '*.[Ccshlxy]' -o -name 'Makefile*' -o -name '*.il*' \
	    -o -name '*.cc' -o -name '*.adb' \) \
	    -print > cscope.files.raw
	grep -v Makefile cscope.files.raw >> cscope.files
	grep Makefile cscope.files.raw >> cscope.files
	-$(RM) cscope.files.raw
	@wc -l cscope.files
E 4
E 1
