h18715
s 00059/00019/00043
d D 1.2 01/10/23 00:05:29 bmc 3 1
c 1000017 Ernie needs levels other than "Hard" and "Impossible"
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:16 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/Makefile
c Name history : 1 0 src/widget/Makefile
e
s 00062/00000/00000
d D 1.1 01/10/20 13:35:15 bmc 1 0
c date and time created 01/10/20 13:35:15 by bmc
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
#     DATE: 4May94
#    DESCR: Makefile for BattleTris Motif Widget library
# MODIFIED: 31Oct95
#

include ../Makeinclude

LIBNAME	= BTWidget
LIBRARY	= lib$(LIBNAME).a

D 3
LIBOBJ	= BTWidget.o BTDrawingAreaWidget.o BTFormWidget.o BTLabelWidget.o \
	BTMessageDlog.o BTPushButtonWidget.o BTRowColumnWidget.o \
	BTScrolledListWidget.o BTScrolledTextWidget.o BTTextWidget.o \
	BTPixmap.o BTFrameWidget.o BTDisplay.o BTXDisplay.o BTXPalette.o \
E 3
I 3
LIBOBJ	= \
	BTWidget.o \
	BTDrawingAreaWidget.o \
	BTFormWidget.o \
	BTLabelWidget.o \
	BTMessageDlog.o \
	BTPushButtonWidget.o \
	BTRowColumnWidget.o \
	BTSliderWidget.o \
	BTScrolledListWidget.o \
	BTScrolledTextWidget.o \
	BTTextWidget.o \
	BTPixmap.o \
	BTFrameWidget.o \
	BTDisplay.o \
	BTXDisplay.o \
	BTXPalette.o \
E 3
	BTXmUtils.o

D 3
LIBINC	= BTDrawingAreaWidget.H BTFormWidget.H \
	BTLabelWidget.H BTMessageDlog.H BTPushButtonWidget.H \
	BTRowColumnWidget.H BTScrolledListWidget.H BTScrolledTextWidget.H \
	BTStatusDlog.H BTTextWidget.H BTFrameWidget.H BTCheckBoxWidget.H \
	BTWidget.H BTPixmap.H BTDisplay.H BTXDisplay.H BTPalette.H BTColor.H \
	BTXColor.H BTXPalette.H
E 3
I 3
LIBINC	= \
	BTDrawingAreaWidget.H \
	BTFormWidget.H \
	BTLabelWidget.H \
	BTMessageDlog.H \
	BTPushButtonWidget.H \
	BTRowColumnWidget.H \
	BTSliderWidget.H \
	BTScrolledListWidget.H \
	BTScrolledTextWidget.H \
	BTStatusDlog.H \
	BTTextWidget.H \
	BTFrameWidget.H \
	BTCheckBoxWidget.H \
	BTWidget.H \
	BTPixmap.H \
	BTDisplay.H \
	BTXDisplay.H \
	BTPalette.H \
	BTColor.H \
	BTXColor.H \
	BTXPalette.H
E 3

DSTLIB	= $(DSTLIBDIR)/$(LIBRARY)
D 3
DSTINC	= $(DSTINCDIR)/BTDrawingAreaWidget.H $(DSTINCDIR)/BTFormWidget.H \
E 3
I 3
DSTINC	= \
	$(DSTINCDIR)/BTDrawingAreaWidget.H \
	$(DSTINCDIR)/BTFormWidget.H \
E 3
	$(DSTINCDIR)/BTLabelWidget.H \
D 3
	$(DSTINCDIR)/BTMessageDlog.H $(DSTINCDIR)/BTPushButtonWidget.H \
	$(DSTINCDIR)/BTRowColumnWidget.H $(DSTINCDIR)/BTScrolledListWidget.H \
	$(DSTINCDIR)/BTScrolledTextWidget.H $(DSTINCDIR)/BTStatusDlog.H \
	$(DSTINCDIR)/BTTextWidget.H $(DSTINCDIR)/BTFrameWidget.H \
	$(DSTINCDIR)/BTCheckBoxWidget.H $(DSTINCDIR)/BTWidget.H \
	$(DSTINCDIR)/BTDisplay.H $(DSTINCDIR)/BTXDisplay.H \
	$(DSTINCDIR)/BTPalette.H $(DSTINCDIR)/BTColor.H \
	$(DSTINCDIR)/BTXColor.H $(DSTINCDIR)/BTXPalette.H \
E 3
I 3
	$(DSTINCDIR)/BTMessageDlog.H \
	$(DSTINCDIR)/BTPushButtonWidget.H \
	$(DSTINCDIR)/BTRowColumnWidget.H \
	$(DSTINCDIR)/BTScrolledListWidget.H \
	$(DSTINCDIR)/BTScrolledTextWidget.H \
	$(DSTINCDIR)/BTSliderWidget.H \
	$(DSTINCDIR)/BTStatusDlog.H \
	$(DSTINCDIR)/BTTextWidget.H \
	$(DSTINCDIR)/BTFrameWidget.H \
	$(DSTINCDIR)/BTCheckBoxWidget.H \
	$(DSTINCDIR)/BTWidget.H \
	$(DSTINCDIR)/BTDisplay.H \
	$(DSTINCDIR)/BTXDisplay.H \
	$(DSTINCDIR)/BTPalette.H \
	$(DSTINCDIR)/BTColor.H \
	$(DSTINCDIR)/BTXColor.H \
	$(DSTINCDIR)/BTXPalette.H \
E 3
	$(DSTINCDIR)/BTPixmap.H

IFLAGS	= $(BT_IFLAGS)
LDFLAGS	= $(BT_LDFLAGS) $(MOTIF_LIBS) $(X11_LIBS)
CXXFLAGS= $(BT_CXXFLAGS) $(MOTIF_CFLAGS) $(X11_CFLAGS)

all:	$(LIBRARY)

$(DSTLIB): $(LIBRARY)
	$(INSTALL) -m 0444 $(LIBRARY) $@

$(DSTINC): $$(@F)
	$(INSTALL) -m 0444 $(@F) $@

$(LIBRARY): $(LIBOBJ)
	$(AR) $@ $?
	$(RANLIB) $@

.C.o:
	$(CXX) $(CXXFLAGS) $(IFLAGS) -c $<

clean:
	$(RM) $(LIBRARY) $(LIBOBJ) core

install: $(DSTLIB) $(DSTINC)
E 1
