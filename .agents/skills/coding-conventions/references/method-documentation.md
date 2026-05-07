# Method and Function Documentation

This convention requires adding **lean, descriptive comments** above every method or function to document its purpose and behavior. The goal is to make the codebase easily understandable for both human developers and AI coding agents.

## Instructions

1. **Always document**: Place a comment above every method/function you implement, regardless of its size or simplicity.

2. **Prioritize language conventions**: Use the **native documentation style** of the target language:
   - **TypeScript/JavaScript**: Use JSDoc (`/** ... */`)
   - **Python**: Use triple-quoted strings (""" ... """)
   - **Rust**: Use `///` for documentation comments
   - **Java/C#**: Use XMLDoc (`/// <summary>...</summary>` or `/** ... */`)
   - **Go**: Use `//` comments directly above the function

3. **Content requirements**: Each comment should include:
   - A brief description of **what the method does** (1 sentence)
   - Key **parameters** and their purpose (if not obvious)
   - **Return value** description (if applicable)
   - Any **side effects** or important behaviors

4. **Keep it lean**: Be concise but descriptive. Aim for 1-3 sentences. Avoid restating what the code obviously does.

5. **For AI agents**: Include sufficient context so AI can understand the method's purpose without reading the implementation.

## Examples

### TypeScript
```typescript
/**
 * Calculates the total price of a cart including tax.
 * 
 * @param cartItems - Array of items with price and quantity
 * @param taxRate - Tax rate as a decimal (e.g., 0.08 for 8%)
 * @returns Total price after applying tax
 */
function calculateTotal(cartItems: CartItem[], taxRate: number): number {
  const subtotal = cartItems.reduce((sum, item) => sum + (item.price * item.quantity), 0);
  return subtotal * (1 + taxRate);
}
```

### Python
```python
def calculate_total(cart_items: list[CartItem], tax_rate: float) -> float:
    """
    Calculate the total price of a cart including tax.
    
    Args:
        cart_items: List of items with price and quantity
        tax_rate: Tax rate as a decimal (e.g., 0.08 for 8%)
    
    Returns:
        Total price after applying tax
    """
    subtotal = sum(item.price * item.quantity for item in cart_items)
    return subtotal * (1 + tax_rate)
```

### Rust
```rust
/// Calculates the total price of a cart including tax.
///
/// # Arguments
/// * `cart_items` - Vector of items with price and quantity
/// * `tax_rate` - Tax rate as a decimal (e.g., 0.08 for 8%)
///
/// # Returns
/// Total price after applying tax
fn calculate_total(cart_items: Vec<CartItem>, tax_rate: f64) -> f64 {
    let subtotal: f64 = cart_items.iter().map(|item| item.price * item.quantity as f64).sum();
    subtotal * (1.0 + tax_rate)
}
```

### Java
```java
/**
 * Calculates the total price of a cart including tax.
 * 
 * @param cartItems Array of items with price and quantity
 * @param taxRate Tax rate as a decimal (e.g., 0.08 for 8%)
 * @return Total price after applying tax
 */
public static double calculateTotal(CartItem[] cartItems, double taxRate) {
    double subtotal = Arrays.stream(cartItems)
        .mapToDouble(item -> item.getPrice() * item.getQuantity())
        .sum();
    return subtotal * (1 + taxRate);
}
```

### Go
```go
// calculateTotal calculates the total price of a cart including tax.
// Parameters:
//   cartItems - slice of items with price and quantity
//   taxRate - tax rate as a decimal (e.g., 0.08 for 8%)
// Returns:
//   Total price after applying tax
func calculateTotal(cartItems []CartItem, taxRate float64) float64 {
	var subtotal float64
	for _, item := range cartItems {
		subtotal += item.Price * float64(item.Quantity)
	}
	return subtotal * (1 + taxRate)
}
```

## Constraints

- **Do NOT** use language-specific doc syntax for a different language (e.g., don't use JSDoc for Python)
- **Do NOT** omit documentation for "simple" or "obvious" methods
- **Do NOT** make comments so verbose they repeat the implementation line-by-line
- **Prefer** clarity over brevity - err on the side of including more context rather than less
