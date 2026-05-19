#
#     NAME: Makefile
#   AUTHOR: Michael Shapiro
#     DATE: 13Oct94
#    DESCR: Makefile for BattleTris signals library
# MODIFIED: 26Oct95
#

include ../Makeinclude

LIBNAME	= BTSignals
LIBRARY	= lib$(LIBNAME).a

LIBOBJ	= SigReceiver.o
LIBINC	= SigHandler.H SigReceiver.H

DSTLIB	= $(DSTLIBDIR)/$(LIBRARY)
DSTINC	= $(DSTINCDIR)/SigHandler.H $(DSTINCDIR)/SigReceiver.H

IFLAGS	= $(BT_IFLAGS)
LDFLAGS	= $(BT_LDFLAGS)
CXXFLAGS= $(BT_CXXFLAGS)

all:	$(LIBRARY)

$(DSTLIB): $(LIBRARY)
	$(INSTALL) -m 0444 $(LIBRARY) $@

$(LIBRARY): $(LIBOBJ)
	$(AR) $@ $?
	$(RANLIB) $@

.C.o:
	$(CXX) $(CXXFLAGS) $(IFLAGS) -c $<

clean:
	$(RM) $(LIBRARY) $(LIBOBJ) core

install: $(DSTLIB) $(DSTINC)
