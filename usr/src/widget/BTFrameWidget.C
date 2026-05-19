#include "BTConfig.H"
#include "BTFrameWidget.H"

BTFrameWidget::BTFrameWidget(BTWidget *parent, const char *name,
    Position x1, Position y1, Position x2, Position y2)
: BTWidget(parent)
{
	me_ = XtVaCreateWidget(name, xmFrameWidgetClass,
	    parent ? parent->getWidget() : (Widget) NULL,
	    XmNx, x1,
	    XmNy, y1,
	    XmNwidth, x2-x1,
	    XmNheight, y2-y1,
	    XmNmarginHeight, 3, XmNmarginWidth, 3,
	    XmNshadowType, XmSHADOW_ETCHED_IN, NULL);
}

BTFrameWidget::BTFrameWidget(BTWidget *parent, const char *name)
: BTWidget(parent)
{
	me_ = XtVaCreateWidget(name, xmFrameWidgetClass,
	    parent ? parent->getWidget() : (Widget) NULL,
	    XmNmarginHeight, 3, XmNmarginWidth, 3,
	    XmNshadowType, XmSHADOW_ETCHED_IN, NULL);
}
