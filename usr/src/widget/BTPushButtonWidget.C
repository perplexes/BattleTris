/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTPushButtonWidget.C                                */
/*    ASGN: Images                                              */
/*    DATE: Tue Apr 19 20:09:00 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTXmUtils.H"
#include "BTPushButtonWidget.H"
#include "BattleTris.H"

BTPushButtonWidget::BTPushButtonWidget(BTWidget *parent, const char *name,
    const char *title)
: BTWidget(parent), activate_struct_(0)
{
	XmString str = xm_strcreate(title);
	me_ = XtVaCreateWidget(name, xmPushButtonWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNlabelString, str,
	    XmNdepth, g_depth,
	    XmNvisual, g_visual,
	    XmNcolormap, g_colormap,
	    NULL);
	XmStringFree(str);
	XtAddCallback(me_, XmNactivateCallback, activate_CB, (XtPointer)this);
}

BTPushButtonWidget::BTPushButtonWidget(BTWidget *parent, const char *name,
    const char *title, Position x1, Position y1, Position x2, Position y2)
: BTWidget(parent), activate_struct_(0) 
{
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);

	me_ = XtVaCreateWidget(name, xmPushButtonWidgetClass,
	    parent ? parent->getWidget() : (Widget) NULL,
	    XmNlabelString, str,
	    XmNdepth, g_depth,
	    XmNvisual, g_visual,
	    XmNcolormap, g_colormap,
	    NULL);

	size(x1, y1, x2 - x1, y2 - y1);
	XmStringFree(str);

	XtAddCallback(me_, XmNactivateCallback, activate_CB, (XtPointer)this);
}

BTPushButtonWidget::BTPushButtonWidget(BTWidget *parent, const char *name,
    const char *title, Dimension width, Dimension height)
: BTWidget(parent), activate_struct_(0) 
{
	XmString str =
	    XmStringCreateLtoR((char *)title, XmFONTLIST_DEFAULT_TAG);
	me_  = XtVaCreateWidget(name, xmPushButtonWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNlabelString, str,
	    XmNdepth, g_depth,
	    XmNvisual, g_visual,
	    XmNcolormap, g_colormap,
	    NULL);

	size((Dimension)-1, (Dimension)-1, width, height );
	XmStringFree(str);

	XtAddCallback(me_, XmNactivateCallback, activate_CB, (XtPointer)this);
}

void
BTPushButtonWidget::setLabel(const char *new_label)
{
	XmString str =
	    XmStringCreateLtoR((char *)new_label, XmFONTLIST_DEFAULT_TAG);
	XtVaSetValues(me_, XmNlabelString, str, NULL);
	XmStringFree(str);
}
