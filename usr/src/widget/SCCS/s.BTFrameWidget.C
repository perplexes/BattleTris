h20212
s 00015/00017/00010
d D 1.2 01/10/21 19:25:15 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:19 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTFrameWidget.C
c Name history : 1 0 src/widget/BTFrameWidget.C
e
s 00027/00000/00000
d D 1.1 01/10/20 13:35:18 bmc 1 0
c date and time created 01/10/20 13:35:18 by bmc
e
u
U
f e 0
t
T
I 1
#include "BTConfig.H"
#include "BTFrameWidget.H"

D 3
BTFrameWidget::BTFrameWidget(BTWidget *parent, char *const name,
                             Position x1, Position y1, Position x2, Position y2)
E 3
I 3
BTFrameWidget::BTFrameWidget(BTWidget *parent, const char *name,
    Position x1, Position y1, Position x2, Position y2)
E 3
: BTWidget(parent)
{
D 3
  me_ =
    XtVaCreateWidget(name, xmFrameWidgetClass,
                     parent ? parent->getWidget() : (Widget) NULL,
		     XmNx, x1,
		     XmNy, y1,
		     XmNwidth, x2-x1,
		     XmNheight, y2-y1,
                     XmNmarginHeight, 3, XmNmarginWidth, 3,
                     XmNshadowType, XmSHADOW_ETCHED_IN, NULL);
E 3
I 3
	me_ = XtVaCreateWidget(name, xmFrameWidgetClass,
	    parent ? parent->getWidget() : (Widget) NULL,
	    XmNx, x1,
	    XmNy, y1,
	    XmNwidth, x2-x1,
	    XmNheight, y2-y1,
	    XmNmarginHeight, 3, XmNmarginWidth, 3,
	    XmNshadowType, XmSHADOW_ETCHED_IN, NULL);
E 3
}

D 3
BTFrameWidget::BTFrameWidget(BTWidget *parent, char *const name)
E 3
I 3
BTFrameWidget::BTFrameWidget(BTWidget *parent, const char *name)
E 3
: BTWidget(parent)
{
D 3
  me_ =
    XtVaCreateWidget(name, xmFrameWidgetClass,
                     parent ? parent->getWidget() : (Widget) NULL,
                     XmNmarginHeight, 3, XmNmarginWidth, 3,
                     XmNshadowType, XmSHADOW_ETCHED_IN, NULL);
E 3
I 3
	me_ = XtVaCreateWidget(name, xmFrameWidgetClass,
	    parent ? parent->getWidget() : (Widget) NULL,
	    XmNmarginHeight, 3, XmNmarginWidth, 3,
	    XmNshadowType, XmSHADOW_ETCHED_IN, NULL);
E 3
}
E 1
