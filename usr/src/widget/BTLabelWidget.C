/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTLabelWidget.C                                     */
/*    ASGN: Final                                               */
/*    DATE: Thu Apr 21 14:07:37 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTLabelWidget.H"

BTLabelWidget::BTLabelWidget(BTWidget *parent, const char *name,
    const char *title)
: BTWidget(parent)
{
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);
	me_  = XtVaCreateWidget(name, xmLabelWidgetClass,
	    parent ? parent->getWidget() : 0, XmNlabelString, str, NULL);
	XmStringFree(str);
	setupStructs();
}

BTLabelWidget::BTLabelWidget(BTWidget *parent, const char *name,
   const char *title, Position x1, Position y1, Position x2, Position y2)
: BTWidget(parent)
{
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);
	me_  = XtVaCreateWidget(name, xmLabelWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNlabelString, str,
	    XmNtopAttachment, XmATTACH_POSITION, XmNtopPosition, y1,
	    XmNleftAttachment, XmATTACH_POSITION, XmNleftPosition, x1,
	    XmNwidth, x2 - x1 + 1, XmNheight, y2 - y1 + 1,
	    XmNresizePolicy, XmRESIZE_NONE, NULL);
	XmStringFree(str);
	setupStructs();
}

BTLabelWidget::BTLabelWidget(BTWidget *parent, const char *name,
    const char *title, Dimension width, Dimension height)
: BTWidget(parent)
{
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);
	me_ = XtVaCreateWidget(name, xmLabelWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNlabelString, str,
	    XmNwidth, width, XmNheight, height,
	    XmNresizePolicy, XmRESIZE_NONE,
	    NULL);
	XmStringFree(str);
	setupStructs();
}

void
BTLabelWidget::setLabel(const char *new_label)
{
	XmString str =
	    XmStringCreateLtoR((char *)new_label, XmFONTLIST_DEFAULT_TAG);
	XtVaSetValues(me_, XmNlabelString, (char *)str, NULL);
	XmStringFree(str);
}

void
BTLabelWidget::setupStructs()
{
	input_struct_ = 0;
	XtAddEventHandler(me_, ButtonPressMask, FALSE, input_CB, this);
}
