/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTFormWidget.C                                      */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 18:02:23 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTFormWidget.H"
#include "BattleTris.H"

BTFormWidget::BTFormWidget(BTWidget *parent, const char *name,
    Dimension width, Dimension height, int fraction_base)
: BTWidget(parent), frac_base_(fraction_base)
{
	me_ = XtVaCreateWidget(name, xmFormWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNheight, height, XmNwidth, width,
	    XmNfractionBase, fraction_base,
	    XmNdepth, g_depth,
	    XmNvisual, g_visual,
	    XmNcolormap, g_colormap,
	    NULL);
}

void
BTFormWidget::placeChild(BTWidget *child, Dimension x, Dimension y)
{
	XtVaSetValues(*child,
	    XmNleftAttachment, XmATTACH_POSITION,
	    XmNleftPosition, x,
	    XmNtopAttachment, XmATTACH_POSITION,
	    XmNtopPosition, y, 0);
}

void
BTFormWidget::placeChild(BTWidget *child, Dimension x1, Dimension y1,
    Dimension x2, Dimension y2)
{
	XtVaSetValues(*child,
	    XmNleftAttachment, XmATTACH_POSITION,
	    XmNleftPosition, x1,
	    XmNtopAttachment, XmATTACH_POSITION,
	    XmNtopPosition, y1,
	    XmNrightAttachment, XmATTACH_POSITION,
	    XmNrightPosition, x2,
	    XmNbottomAttachment, XmATTACH_POSITION,
	    XmNbottomPosition, y2,
	    0);
}
