#
#     NAME: Makefile
#   AUTHOR: Michael Shapiro
#     DATE: 13Oct94
#    DESCR: Makefile for BattleTris audio library
# MODIFIED: 26Oct95
#

include ../Makeinclude

LIBNAME	= BTAudio
LIBRARY	= lib$(LIBNAME).a

LIBOBJ	= DevAudio.o
LIBINC	= DevAudio.H

DSTLIB	= $(DSTLIBDIR)/$(LIBRARY)
DSTINC	= $(DSTINCDIR)/DevAudio.H

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
